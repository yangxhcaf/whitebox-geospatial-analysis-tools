/* 
This tool is part of the WhiteboxTools geospatial analysis library.
Authors: Dr. John Lindsay
Created: June 26, 2017
Last Modified: January 21, 2018
License: MIT
*/

use time;
use num_cpus;
use std::env;
use std::path;
use std::f64;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use raster::*;
use std::io::{Error, ErrorKind};
use tools::*;

pub struct ConservativeSmoothingFilter {
    name: String,
    description: String,
    toolbox: String,
    parameters: Vec<ToolParameter>,
    example_usage: String,
}

impl ConservativeSmoothingFilter {
    pub fn new() -> ConservativeSmoothingFilter { // public constructor
        let name = "ConservativeSmoothingFilter".to_string();
        let toolbox = "Image Processing Tools/Filters".to_string();
        let description = "Performs a conservative-smoothing filter on an image.".to_string();
        
        let mut parameters = vec![];
        parameters.push(ToolParameter{
            name: "Input File".to_owned(), 
            flags: vec!["-i".to_owned(), "--input".to_owned()], 
            description: "Input raster file.".to_owned(),
            parameter_type: ParameterType::ExistingFile(ParameterFileType::Raster),
            default_value: None,
            optional: false
        });

        parameters.push(ToolParameter{
            name: "Output File".to_owned(), 
            flags: vec!["-o".to_owned(), "--output".to_owned()], 
            description: "Output raster file.".to_owned(),
            parameter_type: ParameterType::NewFile(ParameterFileType::Raster),
            default_value: None,
            optional: false
        });

        parameters.push(ToolParameter{
            name: "Filter X-Dimension".to_owned(), 
            flags: vec!["--filterx".to_owned()], 
            description: "Size of the filter kernel in the x-direction.".to_owned(),
            parameter_type: ParameterType::Integer,
            default_value: Some("11".to_owned()),
            optional: true
        });

        parameters.push(ToolParameter{
            name: "Filter Y-Dimension".to_owned(), 
            flags: vec!["--filtery".to_owned()], 
            description: "Size of the filter kernel in the y-direction.".to_owned(),
            parameter_type: ParameterType::Integer,
            default_value: Some("11".to_owned()),
            optional: true
        });

        let sep: String = path::MAIN_SEPARATOR.to_string();
        let p = format!("{}", env::current_dir().unwrap().display());
        let e = format!("{}", env::current_exe().unwrap().display());
        let mut short_exe = e.replace(&p, "").replace(".exe", "").replace(".", "").replace(&sep, "");
        if e.contains(".exe") {
            short_exe += ".exe";
        }
        let usage = format!(">>.*{} -r={} -v --wd=\"*path*to*data*\" -i=image.dep -o=output.dep --filter=25", short_exe, name).replace("*", &sep);
    
        ConservativeSmoothingFilter { 
            name: name, 
            description: description, 
            toolbox: toolbox,
            parameters: parameters, 
            example_usage: usage 
        }
    }
}

impl WhiteboxTool for ConservativeSmoothingFilter {
    fn get_source_file(&self) -> String {
        String::from(file!())
    }
    
    fn get_tool_name(&self) -> String {
        self.name.clone()
    }

    fn get_tool_description(&self) -> String {
        self.description.clone()
    }

    fn get_tool_parameters(&self) -> String {
        match serde_json::to_string(&self.parameters) {
            Ok(json_str) => return format!("{{\"parameters\":{}}}", json_str),
            Err(err) => return format!("{:?}", err),
        }
    }

    fn get_example_usage(&self) -> String {
        self.example_usage.clone()
    }

    fn get_toolbox(&self) -> String {
        self.toolbox.clone()
    }

    fn run<'a>(&self, args: Vec<String>, working_directory: &'a str, verbose: bool) -> Result<(), Error> {
        let mut input_file = String::new();
        let mut output_file = String::new();
        let mut filter_size_x = 11usize;
        let mut filter_size_y = 11usize;
        if args.len() == 0 {
            return Err(Error::new(ErrorKind::InvalidInput,
                                "Tool run with no paramters."));
        }
        for i in 0..args.len() {
            let mut arg = args[i].replace("\"", "");
            arg = arg.replace("\'", "");
            let cmd = arg.split("="); // in case an equals sign was used
            let vec = cmd.collect::<Vec<&str>>();
            let mut keyval = false;
            if vec.len() > 1 {
                keyval = true;
            }
            if vec[0].to_lowercase() == "-i" || vec[0].to_lowercase() == "--input" {
                if keyval {
                    input_file = vec[1].to_string();
                } else {
                    input_file = args[i+1].to_string();
                }
            } else if vec[0].to_lowercase() == "-o" || vec[0].to_lowercase() == "--output" {
                if keyval {
                    output_file = vec[1].to_string();
                } else {
                    output_file = args[i+1].to_string();
                }
            } else if vec[0].to_lowercase() == "-filter" || vec[0].to_lowercase() == "--filter" {
                if keyval {
                    filter_size_x = vec[1].to_string().parse::<usize>().unwrap();
                } else {
                    filter_size_x = args[i+1].to_string().parse::<usize>().unwrap();
                }
                filter_size_y = filter_size_x;
            } else if vec[0].to_lowercase() == "-filterx" || vec[0].to_lowercase() == "--filterx" {
                if keyval {
                    filter_size_x = vec[1].to_string().parse::<usize>().unwrap();
                } else {
                    filter_size_x = args[i+1].to_string().parse::<usize>().unwrap();
                }
            } else if vec[0].to_lowercase() == "-filtery" || vec[0].to_lowercase() == "--filtery" {
                if keyval {
                    filter_size_y = vec[1].to_string().parse::<usize>().unwrap();
                } else {
                    filter_size_y = args[i+1].to_string().parse::<usize>().unwrap();
                }
            }
        }

        if verbose {
            println!("***************{}", "*".repeat(self.get_tool_name().len()));
            println!("* Welcome to {} *", self.get_tool_name());
            println!("***************{}", "*".repeat(self.get_tool_name().len()));
        }

        let sep: String = path::MAIN_SEPARATOR.to_string();

        if filter_size_x < 3 { filter_size_x = 3; }
        if filter_size_y < 3 { filter_size_y = 3; }

        // The filter dimensions must be odd numbers such that there is a middle pixel
        if (filter_size_x as f64 / 2f64).floor() == (filter_size_x as f64 / 2f64) {
            filter_size_x += 1;
        }
        if (filter_size_y as f64 / 2f64).floor() == (filter_size_y as f64 / 2f64) {
            filter_size_y += 1;
        }

        // let (mut z, mut z_n): (f64, f64);
        let midpoint_x = (filter_size_x as f64 / 2f64).floor() as isize;
        let midpoint_y = (filter_size_y as f64 / 2f64).floor() as isize;
        let mut progress: usize;
        let mut old_progress: usize = 1;

        if !input_file.contains(&sep) && !input_file.contains("/") {
            input_file = format!("{}{}", working_directory, input_file);
        }
        if !output_file.contains(&sep) && !output_file.contains("/") {
            output_file = format!("{}{}", working_directory, output_file);
        }

        if verbose { println!("Reading data...") };

        let input = Arc::new(Raster::new(&input_file, "r")?);

        let start = time::now();

        let mut output = Raster::initialize_using_file(&output_file, &input);
        let rows = input.configs.rows as isize;

        let num_procs = num_cpus::get() as isize;
        let (tx, rx) = mpsc::channel();
        for tid in 0..num_procs {
            let input = input.clone();
            let tx1 = tx.clone();
            thread::spawn(move || {
                let nodata = input.configs.nodata;
                let columns = input.configs.columns as isize;
                let (mut z_n, mut z) : (f64, f64);
                let (mut min_val, mut max_val): (f64, f64);
                let (mut min_val2, mut max_val2): (f64, f64);
                let (mut start_col, mut end_col, mut start_row, mut end_row): (isize, isize, isize, isize);
                for row in (0..rows).filter(|r| r % num_procs == tid) {
                    let mut filter_min_vals: VecDeque<f64> = VecDeque::with_capacity(filter_size_x);
                    let mut filter_max_vals: VecDeque<f64> = VecDeque::with_capacity(filter_size_x);
                    start_row = row - midpoint_y;
                    end_row = row + midpoint_y;
                    let mut data = vec![nodata; columns as usize];
                    for col in 0..columns {
                        if col > 0 {
                            filter_min_vals.pop_front();
                            filter_max_vals.pop_front();
                            min_val = f64::INFINITY;
                            max_val = f64::NEG_INFINITY;
                            for row2 in start_row..end_row+1 {
                                z_n = input.get_value(row2, col + midpoint_x);
                                if z_n != nodata {
                                    if z_n < min_val { min_val = z_n; }
                                    if z_n > max_val { max_val = z_n; }
                                }
                            }
                            filter_min_vals.push_back(min_val);
                            filter_max_vals.push_back(max_val);
                        } else {
                            // initialize the filter_vals
                            start_col = col - midpoint_x;
                            end_col = col + midpoint_x;
                            for col2 in start_col..end_col+1 {
                                min_val = f64::INFINITY;
                                max_val = f64::NEG_INFINITY;
                                for row2 in start_row..end_row+1 {
                                    z_n = input[(row2, col2)];
                                    if z_n != nodata {
                                        if z_n < min_val { min_val = z_n; }
                                        if z_n > max_val { max_val = z_n; }
                                    }
                                }
                                filter_min_vals.push_back(min_val);
                                filter_max_vals.push_back(max_val);
                            }
                        }
                        z = input[(row, col)];
                        if z != nodata {
                            min_val = f64::INFINITY;
                            max_val = f64::NEG_INFINITY;
                            min_val2 = min_val;
                            max_val2 = max_val;
                            for i in 0..filter_size_x {
                                if filter_min_vals[i] < min_val { 
                                    min_val2 = min_val;
                                    min_val = filter_min_vals[i]; 
                                }
                                if filter_max_vals[i] > max_val { 
                                    max_val2 = max_val;
                                    max_val = filter_max_vals[i]; 
                                }
                            }
                            if z > min_val && z < max_val {
                                data[col as usize] = z;
                            } else if z == min_val {
                                if min_val2 != f64::INFINITY {
                                    data[col as usize] = min_val2;
                                } else {
                                    // this should only occur when there is no range of values within the window
                                    data[col as usize] = min_val;
                                }
                            } else if z == max_val {
                                if max_val2 != f64::NEG_INFINITY {
                                    data[col as usize] = max_val2;
                                } else {
                                    // this should only occur when there is no range of values within the window
                                    data[col as usize] = max_val;
                                }
                            }
                        }
                    }
                    tx1.send((row, data)).unwrap();
                }
            });
        }

        for row in 0..rows {
            let data = rx.recv().unwrap();
            output.set_row_data(data.0, data.1);
            if verbose {
                progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
                if progress != old_progress {
                    println!("Progress: {}%", progress);
                    old_progress = progress;
                }
            }
        }

        let end = time::now();
        let elapsed_time = end - start;
        output.add_metadata_entry(format!("Created by whitebox_tools\' {} tool", self.get_tool_name()));
        output.add_metadata_entry(format!("Input file: {}", input_file));
        output.add_metadata_entry(format!("Filter size x: {}", filter_size_x));
        output.add_metadata_entry(format!("Filter size y: {}", filter_size_y));
        output.add_metadata_entry(format!("Elapsed Time (excluding I/O): {}", elapsed_time).replace("PT", ""));

        if verbose { println!("Saving data...") };
        let _ = match output.write() {
            Ok(_) => if verbose { println!("Output file written") },
            Err(e) => return Err(e),
        };
        if verbose {
            println!("{}", &format!("Elapsed Time (excluding I/O): {}", elapsed_time).replace("PT", ""));
        }

        Ok(())
    }
}