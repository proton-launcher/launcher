use std::{any::Any, collections::HashMap, error::Error, fs::{File, create_dir_all, read_to_string}, io::{BufReader, Read, Write, copy}, path::{Path, PathBuf}, process::Command, env};

use boa::{Context, JsResult, JsString, JsValue, object::{JsObject, Object}, property::Attribute};
use fancy_regex::Regex;
use reqwest::blocking::Client;
use serde_json::Value;
use settings::{SettingManager, Setting};
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
    install_script: Option<String>,
    pre_launch_script: Option<String>,
    main_class: Option<String>,
    classpath: Vec<String>,
    program_arguments: Vec<String>,
    java_arguments: Vec<String>,
    policies: Vec<String>,
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

    fn get_install_script(&self) -> &Option<String> {
        &self.install_script
    }

    fn get_pre_launch_script(&self) -> &Option<String> {
        &self.pre_launch_script
    }

    fn get_classpath(&self) -> Vec<String> {
        let mut classpath = Vec::new();
        classpath.append(&mut self.classpath.clone());

        if let Some(parent) = &self.parent {
            classpath.append(&mut parent.get_classpath());
        }

        classpath
    }

    fn get_program_arguments(&self) -> Vec<String> {
        let mut program_arguments = match &self.parent {
            Some(parent) => parent.get_program_arguments(),
            None => Vec::new()
        };
        program_arguments.append(&mut self.program_arguments.clone());

        program_arguments
    }

    fn get_java_arguments(&self) -> Vec<String> {
        let mut java_arguments = match &self.parent {
            Some(parent) => parent.get_java_arguments(),
            None => Vec::new()
        };
        java_arguments.append(&mut self.java_arguments.clone());

        java_arguments
    }

    fn get_policies(&self) -> Result<Vec<String>, Box<dyn Error>> {
        let mut policies = match &self.parent {
            Some(parent) => parent.get_policies()?,
            None => Vec::new()
        };

        let mut special_params: HashMap<&str, String> = HashMap::new();
        special_params.insert("files", std::fs::canonicalize(format!("installation/files/{}", self.id))?.to_str().unwrap().to_string());
        special_params.insert("root", std::fs::canonicalize(env::current_dir()?)?.to_str().unwrap().to_string());
        special_params.insert("path", env::var("PATH")?);

        policies.append(&mut self.policies.iter().map(|policy| {
            apply_special_params(&vec![read_to_string(format!("installation/files/{}/{}", self.id, policy)).unwrap()], &special_params)[0].clone()
        }).collect());

        Ok(policies)
    }
}

fn apply_special_params(arguments: &Vec<String>, special_params: &HashMap<&str, String>) -> Vec<String> {
    arguments.iter().map(|argument| {
        let mut new_argument = argument.clone();
        for (special_param, special_value) in special_params {
            new_argument = new_argument.replace(format!("{{{}}}", special_param).as_str(), special_value);
        }

        new_argument
    }).collect()
}

pub fn download_installation(id: String) -> Result<(), Box<dyn  Error>> {
    let base_url = format!("https://raw.githubusercontent.com/proton-launcher/asset/main/installation/{}", id);
    let files_url = format!("{}/files", base_url);

    let parent_folder = format!("installation/files/{}", id);
    let _ = create_dir_all(&parent_folder);
    
    let client = Client::new();
    
    let files = client.get(files_url).send()?.text()?;
    let files: Vec<&str> = files.split("\n").collect();
    for file in files {
        if file.is_empty() { continue; }
        let url = format!("{}/{}", base_url, file);
        let text = client.get(url).send()?.text()?;

        let mut file = File::create(format!("{}/{}", parent_folder, file))?;
        file.write_all(text.as_bytes())?;
    }
    
    let mut info_json = String::new();

    let mut file = File::open(format!("{}/info.json", parent_folder))?;
    file.read_to_string(&mut info_json)?;

    let info_json: Value = serde_json::from_str(&info_json)?;
    match info_json["parent"].as_str() {
        Some(parent) => {
            download_installation(parent.to_string())?;
        },
        None => (),
    };

    Ok(())
}

pub fn parse_installation(id: String) -> Result<Installation, Box<dyn Error>> {
    let files_directory = format!("installation/files/{}", id);
    let info_file = format!("{}/info.json", files_directory);
    let info_file = File::open(info_file)?;
    let info_file_json: Value = serde_json::from_reader(BufReader::new(info_file))?;
    let game_json = &info_file_json["game"];

    let parent = match info_file_json["parent"].as_str() {
        Some(parent) => Some(Box::new(parse_installation(parent.to_string())?)),
        None => None
    };

    let id = info_file_json["id"].as_str().ok_or("ID not found")?.to_string();

    let install_script = match game_json["install_script"].as_str() {
        Some(install_script) => Some(read_to_string(format!("{}/{}", files_directory, install_script.to_string()))?),
        None => None,
    };

    let pre_launch_script = match game_json["pre_launch_script"].as_str() {
        Some(pre_launch_script) => Some(read_to_string(format!("{}/{}", files_directory, pre_launch_script.to_string()))?),
        None => None,
    };
    
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

    let mut special_params: HashMap<&str, String> = HashMap::new();
    special_params.insert("files", std::fs::canonicalize(format!("installation/files/{}", id))?.to_str().unwrap().to_string());
    special_params.insert("root", std::fs::canonicalize(env::current_dir()?)?.to_str().unwrap().to_string());
    special_params.insert("path", env::var("PATH")?);

    let mut program_arguments = Vec::new();
    if let Some(program_arguments_array) = game_json["program_arguments"].as_array() {
        for argument in program_arguments_array {
            program_arguments.push(argument.as_str().unwrap().to_string());
        }
    }
    program_arguments = apply_special_params(&program_arguments, &special_params);

    let mut java_arguments = Vec::new();
    if let Some(java_arguments_array) = game_json["java_arguments"].as_array() {
        for argument in java_arguments_array {
            java_arguments.push(argument.as_str().unwrap().to_string());
        }
    }
    java_arguments = apply_special_params(&java_arguments, &special_params);

    let policies = match game_json["policies"].as_array() {
        Some(policies) => policies.iter().map(|policy_file| {
            policy_file.as_str().unwrap().to_string()
        }).collect(),
        None => Vec::new()
    };

    Ok(Installation {
        parent,
        id,
        install_script,
        pre_launch_script,
        main_class,
        classpath,
        program_arguments,
        java_arguments,
        policies,
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

fn write(_: &JsValue, args: &[JsValue], context: &mut Context) -> Result<JsValue, JsValue> {
    let installation = context.global_object().get("installation", context)?.as_string().unwrap().as_str().to_string();
    let file = format!("installation/files/{}/{}", installation, args[0].as_string().unwrap().as_str().to_string());
    let text = args[1].as_string().unwrap().as_str().to_string();
    File::create(file).unwrap().write_all(text.as_bytes()).into_result()?;

    Ok(JsValue::Null)
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
    let log = match &args[0] {
        JsValue::String(string) => string.as_str().to_string(),
        JsValue::Boolean(boolean) => if *boolean { "true" } else { "false" }.to_string(),
        _ => "unsupported value!".to_string(),
    };
    println!("{}", log);

    Ok(JsValue::Null)
}

fn substring(_: &JsValue, args: &[JsValue], _context: &mut Context) -> Result<JsValue, JsValue> {
    let string = args[0].as_string().unwrap().as_str().to_string();
    let start = args[1].as_number().unwrap() as i32;
    let end = args[2].as_number().unwrap() as i32;

    Ok(JsValue::String(JsString::from(string.chars().skip(start as usize).take((end - start) as usize).collect::<String>())))
}

fn append(_: &JsValue, args: &[JsValue], _context: &mut Context) -> Result<JsValue, JsValue> {
    let string = args[0].as_string().unwrap().as_str().to_string();
    let added_string = args[1].as_string().unwrap().as_str().to_string();

    Ok(JsValue::String(JsString::from(format!("{}{}", string, added_string))))
}

fn regex_capture(_: &JsValue, args: &[JsValue], _context: &mut Context) -> Result<JsValue, JsValue> {
    let string = args[0].as_string().unwrap().as_str().to_string();
    let regex = args[1].as_string().unwrap().as_str().to_string();

    let regex = Regex::new(&regex).unwrap();

    let captures = regex.captures(&string).unwrap().unwrap();
    let capture = captures.get(1).unwrap();

    Ok(JsValue::String(JsString::from(capture.as_str())))
}

fn copy_file(_: &JsValue, args: &[JsValue], context: &mut Context) -> Result<JsValue, JsValue> {
    let installation = context.global_object().get("installation", context)?.as_string().unwrap().as_str().to_string();
    let input = format!("installation/files/{}/{}", installation, args[0].as_string().unwrap().as_str());
    let output = format!("installation/files/{}/{}", installation, args[1].as_string().unwrap().as_str());

    std::fs::copy(input, output).unwrap();

    Ok(JsValue::Null)
}

pub fn install_installation(installation: &Installation) -> Result<(), Box<dyn Error>> {
    let mut context = Context::new();

    context.register_global_function("download", 0, download).into_result()?;
    context.register_global_function("extract", 0, extract).into_result()?;
    context.register_global_function("read", 0, read).into_result()?;
    context.register_global_function("to_json", 0, to_json).into_result()?;
    context.register_global_function("log", 0, log).into_result()?;
    context.register_global_function("substring", 0, substring).into_result()?;
    context.register_global_function("regex_capture", 0, regex_capture).into_result()?;
    context.register_global_function("copy_file", 0, copy_file).into_result()?;

    context.register_global_property("installation", JsValue::String(JsString::from(installation.id.as_str())), Attribute::all());
    context.register_global_property("os", JsValue::String(OS.into()), Attribute::all());

    if let Some(script) = installation.get_install_script() {
        match context.eval(script) {
            Ok(_) => (),
            Err(error) => return Err(error.display().to_string().into())
        };
    }

    if let Some(parent) = &installation.parent {
        install_installation(parent)?;
    }

    Ok(())
}

pub struct RunArguments {
    pub token: String,
    pub uuid: String,
    pub username: String,

}

fn get_settings_value(settings: &SettingManager, context: &mut Context) -> Result<JsValue, Box<dyn Error>> {
    let object = JsObject::new(Object::new());

    for (id, value) in settings.get_settings() {
        object.set(id.as_str(), match value {
            Setting::Boolean(value) => JsValue::Boolean(*value),
            Setting::Integer(integer) => JsValue::Integer(*integer),
            Setting::StringArray(array) => JsValue::String(array.join(",").into()),
            Setting::Null => JsValue::Null,
        }, false, context).unwrap();
    }

    Ok(JsValue::Object(object))
}

fn run_pre_launch_script(installation: &Installation, settings: &SettingManager) -> Result<(), Box<dyn Error>> {
    let mut context = Context::new();

    //TODO: make all things use the same context setup
    context.register_global_function("download", 0, download).into_result()?;
    context.register_global_function("extract", 0, extract).into_result()?;
    context.register_global_function("read", 0, read).into_result()?;
    context.register_global_function("write", 0, write).into_result()?;
    context.register_global_function("to_json", 0, to_json).into_result()?;
    context.register_global_function("log", 0, log).into_result()?;
    context.register_global_function("substring", 0, substring).into_result()?;
    context.register_global_function("append", 0, append).into_result()?;
    context.register_global_function("regex_capture", 0, regex_capture).into_result()?;
    context.register_global_function("copy_file", 0, copy_file).into_result()?;

    context.register_global_property("installation", JsValue::String(JsString::from(installation.id.as_str())), Attribute::all());
    context.register_global_property("os", JsValue::String(OS.into()), Attribute::all());
    let settings_value = get_settings_value(settings, &mut context)?;
    context.register_global_property("settings", settings_value, Attribute::all());

    if let Some(script) = installation.get_pre_launch_script() {
        match context.eval(script) {
            Ok(_) => (),
            Err(error) => return Err(error.display().to_string().into())
        };
    }

    if let Some(parent) = &installation.parent {
        run_pre_launch_script(parent, settings)?;
    }

    Ok(())
}

pub fn run_installation(installation: &Installation, arguments: RunArguments, settings: &SettingManager) -> Result<(), Box<dyn Error>> {
    run_pre_launch_script(installation, settings)?;

    let policy_text = {
        let mut builder = String::new();
        for policy in installation.get_policies()? {
            builder.push_str(&policy);
        }

        builder
    };

    File::create("policy.policy")?.write_all(policy_text.as_bytes())?;

    let mut process = Command::new("java");

    let mut special_params: HashMap<&str, String> = HashMap::new();
    special_params.insert("access_token", arguments.token);
    special_params.insert("uuid", arguments.uuid);
    special_params.insert("username", arguments.username);

    process.args(apply_special_params(&installation.get_java_arguments(), &special_params));
    process.arg("-Djava.security.manager");
    process.arg("-Djava.security.policy==policy.policy");
    //process.arg("-Djava.security.debug=access");
    process.arg("-DLWJGL_DISABLE_XRANDR=true");
    process.arg("-Dsecurity_location=security.security");

    let path_separator = match OS {
        "windows" => ";",
        _ => ":"
    };
    process.args(["-cp", &format!(".{}{}", path_separator, installation.get_classpath().join(path_separator).as_str()), &installation.get_main_class()]);

    process.args(apply_special_params(&installation.get_program_arguments(), &special_params));
    
    process.spawn()?;

    Ok(())
}
