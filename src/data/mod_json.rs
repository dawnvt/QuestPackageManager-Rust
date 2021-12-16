use std::{
    io::{BufReader, Read},
    path::PathBuf,
};

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use crate::data::{
    dependency::{Dependency, SharedDependency},
    package::SharedPackageConfig,
};

// TODO: Idea for later, maybe some kind of config that stores defaults for the different fields, like description and author?
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModJson {
    /// The Questpatcher version this mod.json was made for
    #[serde(rename(serialize = "_QPVersion", deserialize = "_QPVersion"))]
    pub schema_version: Version,
    /// Name of the mod
    pub name: String,
    /// ID of the mod
    pub id: String,
    /// Author of the mod
    pub author: String,
    /// Optional slot for if you ported a mod
    #[serde(skip_serializing_if = "Option::is_none")]
    pub porter: Option<String>,
    /// Mod version
    pub version: String,
    /// id of the package the mod is for, ex. com.beatgaems.beatsaber
    pub package_id: String,
    /// Version of the package, ex. 1.1.0
    pub package_version: String,
    /// description for the mod
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// optional cover image filename
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_image: Option<String>,
    /// whether or not this qmod is a library or not
    pub is_library: Option<bool>,
    /// list of downloadable dependencies
    pub dependencies: Vec<ModDependency>,
    /// list of files that go in the package's mod folder
    pub mod_files: Vec<String>,
    /// list of files that go in the package's libs folder
    pub library_files: Vec<String>,
    /// list of files that will be copied on the quest
    pub file_copies: Vec<FileCopy>,
    /// list of copy extensions registered for this specific mod
    pub copy_extensions: Vec<CopyExtension>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModDependency {
    /// the version requirement for this dependency
    #[serde(deserialize_with = "cursed_semver_parser::deserialize")]
    pub version_range: VersionReq,
    /// the id of this dependency
    pub id: String,
    /// the download link for this dependency, must satisfy id and version range!
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mod_link: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileCopy {
    /// name of the file in the qmod
    pub name: String,
    /// place where to put it (full path)
    pub destination: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CopyExtension {
    /// the extension to register for
    pub extension: String,
    /// the destination folder these files should be going to
    pub destination: String,
}

pub struct PreProcessingData {
    pub version: String,
    pub mod_id: String,
}

impl ModJson {
    pub fn get_template_name() -> &'static str {
        "mod.template.json"
    }

    pub fn get_result_name() -> &'static str {
        "mod.json"
    }

    pub fn get_template_path() -> std::path::PathBuf {
        std::path::PathBuf::new()
            .join(&Self::get_template_name())
            .canonicalize()
            .unwrap()
    }

    pub fn get_result_path() -> std::path::PathBuf {
        std::path::PathBuf::new()
            .join(&Self::get_result_name())
            .canonicalize()
            .unwrap()
    }

    pub fn read_and_preprocess(preprocess_data: &PreProcessingData) -> Self {
        let mut file =
            std::fs::File::open(Self::get_template_name()).expect("Opening mod.json failed");

        // Get data
        let mut json = String::new();
        file.read_to_string(&mut json).expect("Reading data failed");

        // Pre process
        let processsed = Self::preprocess(json, preprocess_data);

        serde_json::from_str(&processsed).expect("Deserializing package failed")
    }

    fn preprocess(s: String, preprocess_data: &PreProcessingData) -> String {
        /*
        replace_fast(
            &s,
            &HashMap::from([
                ("${version}", preprocess_data.version.as_str()),
                ("${mod_id}", preprocess_data.mod_id.as_str()),
            ]),
        )
        */

        // HACK: temp measure because above code didn't work properly
        s.replace("${version}", preprocess_data.version.as_str())
            .replace("${mod_id}", preprocess_data.mod_id.as_str())
    }

    pub fn read(path: PathBuf) -> ModJson {
        let file = std::fs::File::open(path).expect("Opening mod.json failed");
        let reader = BufReader::new(file);

        serde_json::from_reader(reader).expect("Deserializing package failed")
    }

    pub fn write(&self, path: PathBuf) {
        let file = std::fs::File::create(path).expect("create failed");
        serde_json::to_writer_pretty(file, self).expect("Write failed");
    }

    #[inline]
    pub fn write_default(&self) {
        self.write(PathBuf::from(Self::get_result_name()));
    }
}

impl From<SharedPackageConfig> for ModJson {
    fn from(mut shared_package: SharedPackageConfig) -> Self {
        shared_package
            .restored_dependencies
            // keep if header only is false, or if not defined
            .retain(|dep| !dep.dependency.additional_data.headers_only.unwrap_or(false));

        // downloadable mods links n stuff
        let mods: Vec<ModDependency> = shared_package
            .restored_dependencies
            .iter()
            // Removes any dependency without a qmod link
            .filter(|dep| dep.dependency.additional_data.mod_link.is_some())
            .map(|dep| dep.clone().into())
            .collect();

        // actual direct lib deps
        let mut libs = shared_package
            .restored_dependencies
            .iter()
            // TODO: How to blacklist dependencies such as coremods?
            // We could just query the bmbf core mods list on GH?
            // https://github.com/BMBF/resources/blob/master/com.beatgames.beatsaber/core-mods.json
            // but really the only lib that never is copied over is the modloader, the rest is either a downloaded qmod or just a copied lib
            // even core mods should technically be added via download
            .filter(|lib|

                // Modloader should never be included
                lib.dependency.id != "modloader" && 

                // Only keep libs that aren't downloadable
                !mods.iter().any(|dep| lib.dependency.id == dep.id))
            .map(|dep| dep.get_so_name())
            .collect::<Vec<String>>();
        let is_library = shared_package
            .config
            .info
            .additional_data
            .is_library
            .unwrap_or(false);

        if is_library {
            libs.push(shared_package.config.get_so_name());
        }

        let mod_files = if is_library {
            vec![shared_package.config.get_so_name()]
        } else {
            vec![]
        };

        Self {
            schema_version: Version::new(0, 1, 2),
            name: shared_package.config.info.name.clone(),
            id: shared_package.config.info.id.clone(),
            author: Default::default(),
            porter: None,
            version: shared_package.config.info.version.to_string(),
            package_id: "com.beatgames.beatsaber".to_string(),
            package_version: "*".to_string(),
            description: None,
            cover_image: None,
            is_library: shared_package.config.info.additional_data.is_library,
            dependencies: mods,
            mod_files,
            library_files: libs,
            file_copies: Default::default(),
            copy_extensions: Default::default(),
        }
    }
}

impl From<Dependency> for ModDependency {
    fn from(dep: Dependency) -> Self {
        Self {
            id: dep.id,
            version_range: dep.version_range,
            mod_link: dep.additional_data.mod_link,
        }
    }
}

impl From<SharedDependency> for ModDependency {
    fn from(dep: SharedDependency) -> Self {
        dep.dependency.into()
    }
}
