use serde::{Serialize, Deserialize};
use crate::data::dependency::{Dependency, AdditionalDependencyData};
use crate::data::shared_dependency::{SharedDependency};
use crate::data::shared_package::{SharedPackageConfig};
use std::collections::HashMap;
use semver::{Version};
use owo_colors::*;
use std::io::{Write, Read};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AdditionalPackageData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_linking: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub so_link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_files: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug_so_link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_so_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub styles: Option<Vec<PackageStyle>>
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PackageStyle {
    pub name: String,
    pub so_link: String,
    pub debug_so_link: String
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PackageInfo {
    pub name: String,
    pub id: String,
    pub version: String,
    pub url: Option<String>,
    pub additional_data: AdditionalPackageData
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PackageConfig {
    pub shared_dir: String,
    pub dependencies_dir: String,
    pub info: PackageInfo,
    pub dependencies: Vec<Dependency>,
    pub additional_data: AdditionalDependencyData
}

impl PackageConfig {
    pub fn write(&self)
    {
        let qpm_package = serde_json::to_string_pretty(&self).expect("Serialization failed");

        let mut file = std::fs::File::create("qpm.json").expect("create failed");
        file.write_all(qpm_package.as_bytes()).expect("write failed");
        println!("Package {} Written!", self.info.id);
    }

    pub fn read() -> PackageConfig 
    {
        let mut file = std::fs::File::open("qpm.json").expect("Opening qpm.json failed");
        let mut qpm_package = String::new();
        file.read_to_string(&mut qpm_package).expect("Reading data failed");

        serde_json::from_str::<PackageConfig>(&qpm_package).expect("Deserializing package failed")
    }

    pub fn add_dependency(&mut self, dependency: Dependency)
    {
        let dep = self.get_dependency(&dependency.id);
        match dep {
            Option::Some(_d) => { println!("Not adding dependency {} because it already existed", &dependency.id); },
            Option::None => {
                self.dependencies.push(dependency);
            }
        }
        
    }

    pub fn get_dependency(&mut self, id: &str) -> Option<&mut Dependency>
    {
        for (idx, dependency) in self.dependencies.iter().enumerate()
        {
            if dependency.id.eq(id)
            {
                return self.dependencies.get_mut(idx);
            }
        }
        
        Option::default()
    }

    pub fn remove_dependency(&mut self, id: &str)
    {
        for (idx, dependency) in self.dependencies.iter().enumerate()
        {
            if dependency.id.eq(id)
            {
                println!("removed dependency {}", id);            
                self.dependencies.remove(idx);
                return;
            }
        }

        println!("Not removing dependency {} because it did not exist", id);
    }

    pub fn collect(&self) -> HashMap::<SharedDependency, SharedPackageConfig>
    {
        // fd new vector for storing our values
        let mut collected = HashMap::<SharedDependency, SharedPackageConfig>::new();

        // for every dependency defined in our local package (qpm.json)
        for dependency in self.dependencies.iter() { dependency.collect(&self.info.id, &mut collected); }
        
        collected
    }

    pub fn collapse(&self) -> HashMap::<SharedDependency, SharedPackageConfig>
    {
        // collect our dependencies first
        let mut collapsed = self.collect();
        let collapsed_clone = collapsed.clone();

        collapsed.retain(|shared_dependency, _shared_package|{
            for pair in collapsed_clone.iter() {
                if pair.0.dependency.id.eq(&shared_dependency.dependency.id) && pair.0.get_hash() != shared_dependency.get_hash() {
                    let req1 = cursed_semver_parser::parse(&pair.0.dependency.version_range).expect("Parsing first version range failed");
                    let req2 = cursed_semver_parser::parse(&shared_dependency.dependency.version_range).expect("Parsing second version range failed");

                    let ver1 = Version::parse(&pair.0.version).expect("Parsing first version failed");
                    let ver2 = Version::parse(&shared_dependency.version).expect("Parsing second version failed");
                    
                    let match1 = req1.matches(&ver1) && req2.matches(&ver1);
                    let match2 = req1.matches(&ver2) && req2.matches(&ver2);
                    
                    if match1 && match2
                    {
                        // both are good
                        if ver1 > ver2
                        {
                            // if the first version is larger than the second, then that means this is a bad one, remove second
                            return false;
                        }
                    }
                    else if match1
                    {
                        // just the first is good, remove second
                        return false;
                    }
                    else if match2
                    {
                        // just the second is good, this means do nothing right now, we'll get to the point where it'll be removed
                    }
                    else
                    {
                        // neither is good, this means the config is unusable!
                        println!("Cannot collapse {}, Ranges do not intersect:", shared_dependency.dependency.id.bright_red());
                        println!("Range 1: {} --> {}", &pair.0.dependency.version_range.bright_blue(), &pair.0.version.bright_green());
                        println!("Range 2: {} --> {}", &shared_dependency.dependency.version_range.bright_blue(), &shared_dependency.version.bright_green());
                        println!("Consider running {} and see which packages are using incompatible version ranges", "qpm-rust collect".bright_yellow());
                        std::process::exit(0);
                    }
                }
            }
            true
        });
        
        collapsed
    }
}

impl Default for PackageConfig {
    #[inline]
    fn default() -> PackageConfig {
        PackageConfig {
            shared_dir: "shared".to_string(),
            dependencies_dir: "extern".to_string(),
            info: PackageInfo::default(),
            dependencies: Vec::<Dependency>::default(),
            additional_data: AdditionalDependencyData::default()
        }
    }
}