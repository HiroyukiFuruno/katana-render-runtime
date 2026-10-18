use super::physical_file;
use crate::markdown::chrome_font_identity_probe_tests::ResourceFile;
use resvg::usvg::fontdb::Database;
use std::collections::BTreeSet;

pub(super) fn load_resource_database(resources: &[ResourceFile]) -> Result<Database, String> {
    let mut database = Database::new();
    let mut paths = BTreeSet::new();
    for resource in resources {
        load_resource(&mut database, &mut paths, resource)?;
    }
    Ok(database)
}

fn load_resource(
    database: &mut Database,
    paths: &mut BTreeSet<std::path::PathBuf>,
    resource: &ResourceFile,
) -> Result<(), String> {
    if resource.url.is_empty() || !resource.body_path.is_absolute() {
        return Err("resource must have a URL and absolute body path".to_string());
    }
    let (path, digest, _) = physical_file(&resource.body_path)?;
    if digest != resource.bytes_sha256 {
        return Err("font resource bytes do not match the captured SHA-256".to_string());
    }
    if !paths.insert(path.clone()) {
        return Err("duplicate physical font resource path".to_string());
    }
    database
        .load_font_file(path)
        .map_err(|error| format!("load captured font response: {error}"))
}
