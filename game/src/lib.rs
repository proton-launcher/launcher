use std::{any::Any, collections::HashMap, error::Error, fs::{File, create_dir_all, read_to_string}, io::{BufReader, Read, Write, copy}, path::{Path, PathBuf}, process::Command};

use boa::{Context, JsResult, JsString, JsValue, object::{JsObject, Object}, property::Attribute};
use serde_json::Value;
use zip::ZipArchive;

const OS: &'static str = if cfg!(windows) {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else {
            "linux"
        };

trait IntoResult<T, U> {
    fn into_result(self) -> Result<T, U>;
}

impl IntoResult<(), Box<dyn Error>> for JsResult<()> {
    fn into_result(self) -> Result<(), Box<dyn Error>> {
        match self {
            Ok(_) => Ok(()),
            Err(error) => Err(error.display().to_string().into())
        }
    }
}

impl<T: Any, U: Error> IntoResult<JsValue, JsValue> for Result<T, U> {
    fn into_result(self) -> Result<JsValue, JsValue> {
        match self {
            Ok(_) => Ok(JsValue::Null),
            Err(error) => Err(JsValue::String(JsString::from(error.to_string())))
        }
    }
}

#[derive(Debug)]
pub struct Installation {
    parent: Option<Box<Installation>>,
    id: String,
    install_script: String,
    main_class: Option<String>,
    classpath: Vec<String>,
    program_arguments: Vec<String>,
    java_arguments: Vec<String>,
}

impl Installation {
    fn get_main_class(&self) -> String {
        if let Some(main_class_temp) = self.main_class.clone() {
            Some(main_class_temp)
        } else if let Some(parent) = &self.parent {
            Some(parent.get_main_class())
        } else {
            None
        }.unwrap().clone()
    }

    fn get_classpath(&self) -> Vec<String> {
        let mut classpath = Vec::new();
        classpath.append(&mut self.classpath.clone());

        if let Some(parent) = &self.parent {
            classpath.append(&mut parent.get_classpath());
        }

        classpath
    }
}

pub fn parse_installation(id: String) -> Result<Installation, Box<dyn Error>> {
    let files_directory = format!("installation/files/{}", id);
    let info_file = format!("{}/info.json", files_directory);
    let info_file = File::open(info_file)?;
    let info_file_json: Value = serde_json::from_reader(BufReader::new(info_file))?;
    let game_json = &info_file_json["game"];

    let mut parent = None;
    if let Some(parent_config) = game_json["parent"].as_str() {
        parent = Some(Box::new(parse_installation(parent_config.to_string())?));
    }

    let id = info_file_json["id"].as_str().ok_or("ID not found")?.to_string();
    let install_script = read_to_string(format!("{}/{}", files_directory, info_file_json["install_script"].as_str().ok_or("ID not found")?))?;
    
    let main_class = match game_json["main_class"].as_str() {
        Some(path) => Some(path.to_string()),
        None => None
    };

    let mut classpath = Vec::new();
    if let Some(classpath_array) = game_json["classpath"].as_array() {
        for object in classpath_array {
            let path = object["file"].as_str().unwrap().to_string();
            if object.get("platforms").is_none() || object["platforms"].as_array().unwrap().contains(&Value::String(OS.into())) {
                classpath.push(format!("installation/files/{}/{}", id, path));
            }
        }
    }

    let mut program_arguments = Vec::new();
    if let Some(program_arguments_array) = game_json["program_arguments"].as_array() {
        for argument in program_arguments_array {
            program_arguments.push(argument.as_str().unwrap().to_string());
        }
    }

    let mut java_arguments = Vec::new();
    if let Some(java_arguments_array) = game_json["java_arguments"].as_array() {
        for argument in java_arguments_array {
            java_arguments.push(argument.as_str().unwrap().to_string());
        }
    }

    Ok(Installation {
        parent,
        id,
        install_script,
        main_class,
        classpath,
        program_arguments,
        java_arguments,
    })
}

fn download(_: &JsValue, args: &[JsValue], context: &mut Context) -> Result<JsValue, JsValue> {
    let installation = context.global_object().get("installation", context)?.as_string().unwrap().as_str().to_string();
    let url = args[0].as_string().ok_or("Invalid argument for download")?.to_string();
    let path = args[1].as_string().ok_or("Invalid argument for download")?.to_string();
    let path = format!("installation/files/{}/{}", installation, path);

    let request = match reqwest::blocking::get(url) {
        Ok(response) => response,
        Err(error) => return Err(JsValue::String(JsString::from(error.to_string())))
    };
    let bytes = request.bytes().unwrap();

    let parent_path = Path::new(&path).parent().unwrap();
    create_dir_all(parent_path).unwrap();

    let mut file = File::create(path).unwrap();
    file.write_all(&bytes).unwrap();

    Ok(JsValue::Null)
}

fn extract(_: &JsValue, args: &[JsValue], context: &mut Context) -> Result<JsValue, JsValue> {
    let installation = context.global_object().get("installation", context)?.as_string().unwrap().as_str().to_string();
    let zip = format!("installation/files/{}/{}", installation, &args[0].as_string().unwrap().to_string());
    let location = format!("installation/files/{}/{}", installation, &args[1].as_string().unwrap().to_string());
    match extract_zip(zip.as_str(), location.as_str()) {
        Ok(_) => Ok(JsValue::Null),
        Err(e) => Err(format!("Error extracting file {} to {}: {:?}", zip, location, e).into()),
    }
}

// https://github.com/zip-rs/zip/blob/master/examples/extract.rs
fn extract_zip<'a>(zip: &'a str, out_path: &'a str) -> Result<(), Box<dyn Error>> {
    let fname = std::path::Path::new(zip);
    let file = File::open(&fname)?;

    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut outpath = PathBuf::new();
        outpath.push(out_path);
        outpath.push(archive.by_index(i)?.name());
        let mut file = archive.by_index(i)?;

        if (&*file.name()).ends_with('/') {
            create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    create_dir_all(&p)?;
                }
            }
            let mut outfile = File::create(&outpath)?;
            copy(&mut file, &mut outfile)?;
        }
    };

    Ok(())
}

fn read(_: &JsValue, args: &[JsValue], context: &mut Context) -> Result<JsValue, JsValue> {
    let installation = context.global_object().get("installation", context)?.as_string().unwrap().as_str().to_string();
    let file = format!("installation/files/{}/{}", installation, args[0].as_string().unwrap().as_str().to_string());
    let mut string = String::new();
    File::open(file).unwrap().read_to_string(&mut string).into_result()?;

    Ok(JsValue::String(JsString::from(string)))
}

fn to_json(_: &JsValue, args: &[JsValue], context: &mut Context) -> Result<JsValue, JsValue> {
    let string = args[0].as_string().unwrap().as_str().to_string();
    let json: Value = serde_json::from_str(&string).unwrap();

    Ok(to_js_json_internal(json, context)?)
}

fn to_js_json_internal(json: Value, context: &mut Context) -> Result<JsValue, JsValue> {
    match json {
        Value::Object(values) => {
            let object = JsObject::new(Object::new());
            for (key, value) in values {
                object.set(key, to_js_json_internal(value, context)?, false, context)?;
            }

            Ok(JsValue::Object(object))
        },
        Value::String(string) => {
            Ok(JsValue::String(string.into()))
        }
        Value::Number(number) => {
            Ok(JsValue::Integer(number.as_i64().unwrap() as i32))
        }
        _ => Err(JsValue::String(JsString::from(format!("Json type not handled: {}", json))))
    }
}

fn log(_: &JsValue, args: &[JsValue], _context: &mut Context) -> Result<JsValue, JsValue> {
    let log = args[0].as_string().unwrap().as_str().to_string();
    println!("{}", log);

    Ok(JsValue::Null)
}

fn substring(_: &JsValue, args: &[JsValue], _context: &mut Context) -> Result<JsValue, JsValue> {
    let string = args[0].as_string().unwrap().as_str().to_string();
    let start = args[1].as_number().unwrap() as i32;
    let end = args[2].as_number().unwrap() as i32;

    Ok(JsValue::String(JsString::from(string.chars().skip(start as usize).take((end - start) as usize).collect::<String>())))
}

pub fn install_installation(installation: &Installation) -> Result<(), Box<dyn Error>> {
    let mut context = Context::new();

    context.register_global_function("download", 0, download).into_result()?;
    context.register_global_function("extract", 0, extract).into_result()?;
    context.register_global_function("read", 0, read).into_result()?;
    context.register_global_function("to_json", 0, to_json).into_result()?;
    context.register_global_function("log", 0, log).into_result()?;
    context.register_global_function("substring", 0, substring).into_result()?;

    context.register_global_property("installation", JsValue::String(installation.id.clone().into()), Attribute::all());
    context.register_global_property("os", JsValue::String(OS.into()), Attribute::all());

    match context.eval(installation.install_script.clone()) {
        Ok(_) => (),
        Err(error) => return Err(error.display().to_string().into())
    };

    Ok(())
}

pub struct RunArguments {
    pub token: String,
    pub uuid: String,
    pub username: String,

}

pub fn run_installation(installation: &Installation, arguments: RunArguments) -> Result<(), Box<dyn Error>> {
    let mut process = Command::new("java");

    let mut special_params: HashMap<&str, String> = HashMap::new();
    special_params.insert("files", format!("installation/files/{}/", installation.id));
    special_params.insert("access_token", arguments.token);
    special_params.insert("uuid", arguments.uuid);
    special_params.insert("username", arguments.username);

    fn apply_special_params(arguments: &Vec<String>, special_params: &HashMap<&str, String>) -> Vec<String> {
        arguments.iter().map(|argument| {
            let mut new_argument = argument.clone();
            for (special_param, special_value) in special_params {
                new_argument = new_argument.replace(format!("{{{}}}", special_param).as_str(), special_value);
            }

            new_argument
        }).collect()
    }

    process.args(apply_special_params(&installation.java_arguments, &special_params));

    let path_separator = match OS {
        "windows" => ";",
        _ => ":"
    };
    process.args(["-cp", installation.get_classpath().join(path_separator).as_str(), installation.get_main_class().as_str()]);

    process.args(apply_special_params(&installation.program_arguments, &special_params));
    
    process.spawn()?;

    Ok(())
}
