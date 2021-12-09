use std::{any::Any, collections::HashMap, error::Error, fs::{File, create_dir_all, read_to_string}, io::{BufReader, Read, Write, copy}, path::{Path, PathBuf}, process::Command, env::current_dir};

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
    scripts: HashMap<String, String>,
}

impl Installation {
    fn get_script(&self, id: String) -> Option<&String> {
        self.scripts.get(&id)
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

    let parent = match info_file_json["parent"].as_str() {
        Some(parent) => Some(Box::new(parse_installation(parent.to_string())?)),
        None => None
    };

    let id = info_file_json["id"].as_str().ok_or("ID not found")?.to_string();

    let scripts = match info_file_json["scripts"].as_object() {
        Some(scripts) => {
            let mut map = HashMap::new();

            for (id, location) in scripts {
                map.insert(id.clone(), location.as_str().ok_or("script not string")?.to_string());
            }

            map
        },
        None => HashMap::new()
    };

    Ok(Installation {
        parent,
        id,
        scripts,
    })
}

fn download(_: &JsValue, args: &[JsValue], _: &mut Context) -> Result<JsValue, JsValue> {
    let url = args[0].as_string().ok_or("Invalid argument for download")?.to_string();
    let path = args[1].as_string().ok_or("Invalid argument for download")?.to_string();

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

fn extract(_: &JsValue, args: &[JsValue], _: &mut Context) -> Result<JsValue, JsValue> {
    let zip = &args[0].as_string().unwrap().to_string();
    let location = &args[1].as_string().unwrap().to_string();
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

fn read(_: &JsValue, args: &[JsValue], _: &mut Context) -> Result<JsValue, JsValue> {
    let file = args[0].as_string().unwrap().as_str().to_string();
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

fn replace(_: &JsValue, args: &[JsValue], _context: &mut Context) -> Result<JsValue, JsValue> {
    let string = args[0].as_string().unwrap();
    let initial = args[1].as_string().unwrap().as_str().to_string();
    let wanted = args[2].as_string().unwrap().as_str().to_string();

    Ok(JsValue::String(JsString::from(string.replace(initial.as_str(), wanted.as_str()))))
}

fn regex_capture(_: &JsValue, args: &[JsValue], _context: &mut Context) -> Result<JsValue, JsValue> {
    let string = args[0].as_string().unwrap().as_str().to_string();
    let regex = args[1].as_string().unwrap().as_str().to_string();

    let regex = Regex::new(&regex).unwrap();

    let captures = regex.captures(&string).unwrap().unwrap();
    let capture = captures.get(1).unwrap();

    Ok(JsValue::String(JsString::from(capture.as_str())))
}

fn copy_file(_: &JsValue, args: &[JsValue], _: &mut Context) -> Result<JsValue, JsValue> {
    let input = args[0].as_string().unwrap().as_str();
    let output = args[1].as_string().unwrap().as_str();

    std::fs::copy(input, output).unwrap();

    Ok(JsValue::Null)
}

pub fn install_installation(installation: &Installation) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = &installation.parent {
        install_installation(parent)?;
    }

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
    context.register_global_property("files", format!("installation/files/{}", installation.id), Attribute::all());

    if let Some(script) = installation.get_script("install".to_string()) {
        match context.eval(read_to_string(format!("installation/files/{}/{}", installation.id, script))?) {
            Ok(_) => (),
            Err(error) => return Err(error.display().to_string().into())
        };
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

struct LaunchSetup {
    main_class: Option<String>,
    classpath: Vec<String>,
    program_arguments: Vec<String>,
    java_arguments: Vec<String>,
    policies: Vec<String>,
}

impl Default for LaunchSetup {
    fn default() -> Self {
        LaunchSetup {
            main_class: None,
            classpath: Vec::new(),
            program_arguments: Vec::new(),
            java_arguments: Vec::new(),
            policies: Vec::new(),
        }
    }
}

impl Into<LaunchSetup> for Context {
    fn into(mut self) -> LaunchSetup {
        let mut launch_setup = LaunchSetup::default();

        let main_class = self.global_object().get("main_class", &mut self).unwrap().as_string().unwrap().to_string();
        if !main_class.is_empty() {
            launch_setup.main_class = Some(main_class);
        }

        let classpath = self.global_object().get("classpath", &mut self).unwrap().as_object().unwrap();
        let mut current_index = 0;
        while current_index >= 0 {
            match classpath.get(current_index, &mut self).unwrap().as_string() {
                Some(path) => { 
                    launch_setup.classpath.push(path.to_string());
                    current_index += 1;
                },
                None => current_index = -1,
            }
        }

        let policies = self.global_object().get("policies", &mut self).unwrap().as_object().unwrap();
        let mut current_index = 0;
        while current_index >= 0 {
            match policies.get(current_index, &mut self).unwrap().as_string() {
                Some(contents) => { 
                    launch_setup.policies.push(contents.to_string());
                    current_index += 1;
                },
                None => current_index = -1,
            }
        }

        let java_arguments = self.global_object().get("java_arguments", &mut self).unwrap().as_object().unwrap();
        let mut current_index = 0;
        while current_index >= 0 {
            match java_arguments.get(current_index, &mut self).unwrap().as_string() {
                Some(path) => { 
                    launch_setup.java_arguments.push(path.to_string());
                    current_index += 1;
                },
                None => current_index = -1,
            }
        }

        let program_arguments = self.global_object().get("program_arguments", &mut self).unwrap().as_object().unwrap();
        let mut current_index = 0;
        while current_index >= 0 {
            match program_arguments.get(current_index, &mut self).unwrap().as_string() {
                Some(path) => { 
                    launch_setup.program_arguments.push(path.to_string());
                    current_index += 1;
                },
                None => current_index = -1,
            }
        }

        launch_setup
    }
}

fn run_launch_script(installation: &Installation, settings: &SettingManager) -> Result<Context, Box<dyn Error>> {
    let mut context = match &installation.parent {
        Some(parent) => run_launch_script(parent, settings)?,
        None => {
            let mut context = Context::new();

            context.register_global_function("download", 0, download).into_result()?;
            context.register_global_function("extract", 0, extract).into_result()?;
            context.register_global_function("read", 0, read).into_result()?;
            context.register_global_function("write", 0, write).into_result()?;
            context.register_global_function("to_json", 0, to_json).into_result()?;
            context.register_global_function("log", 0, log).into_result()?;
            context.register_global_function("substring", 0, substring).into_result()?;
            context.register_global_function("append", 0, append).into_result()?;
            context.register_global_function("replace", 0, replace).into_result()?;
            context.register_global_function("regex_capture", 0, regex_capture).into_result()?;
            context.register_global_function("copy_file", 0, copy_file).into_result()?;
        
            context.register_global_property("os", JsValue::String(OS.into()), Attribute::all());
            let settings_value = get_settings_value(settings, &mut context)?;
            context.register_global_property("settings", settings_value, Attribute::all());
            context.register_global_property("root", current_dir()?.canonicalize()?.to_str().unwrap(), Attribute::all());

            context.register_global_property("main_class", "", Attribute::all());
            let base_array = context.eval("[]").unwrap();
            context.register_global_property("classpath", base_array, Attribute::all());
            let base_array = context.eval("[]").unwrap();
            context.register_global_property("policies", base_array, Attribute::all());
            let base_array = context.eval("[]").unwrap();
            context.register_global_property("java_arguments", base_array, Attribute::all());
            let base_array = context.eval("[]").unwrap();
            context.register_global_property("program_arguments", base_array, Attribute::all());

            context
        },
    };
    
    context.register_global_property("installation", JsValue::String(JsString::from(installation.id.as_str())), Attribute::all());
    context.register_global_property("files", format!("installation/files/{}", installation.id), Attribute::all());

    if let Some(script) = installation.get_script("launch".to_string()) {
        match context.eval(read_to_string(format!("installation/files/{}/{}", installation.id, script))?) {
            Ok(_) => (),
            Err(error) => return Err(error.display().to_string().into())
        };
    }

    Ok(context)
}

pub fn run_installation(installation: &Installation, arguments: RunArguments, settings: &SettingManager) -> Result<(), Box<dyn Error>> {
    let launch_setup: LaunchSetup = run_launch_script(installation, settings)?.into();

    let policy_text = {
        let mut builder = String::new();
        for policy in launch_setup.policies {
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

    process.args(apply_special_params(&launch_setup.java_arguments, &special_params));
    process.arg("-Djava.security.manager");
    process.arg("-Djava.security.policy==policy.policy");
    //process.arg("-Djava.security.debug=access");
    process.arg("-DLWJGL_DISABLE_XRANDR=true");
    process.arg("-Dsecurity_location=security.security");

    let path_separator = match OS {
        "windows" => ";",
        _ => ":"
    };
    process.args(["-cp", &format!(".{}{}", path_separator, launch_setup.classpath.join(path_separator).as_str()), &launch_setup.main_class.ok_or("no main class")?]);

    process.args(apply_special_params(&launch_setup.program_arguments, &special_params));

    process.spawn()?;

    Ok(())
}
