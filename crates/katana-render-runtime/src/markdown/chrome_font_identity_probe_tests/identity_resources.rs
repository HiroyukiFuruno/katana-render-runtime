use super::physical_file;
use crate::markdown::chrome_font_identity_probe_tests::ResourceFile;
use resvg::usvg::fontdb::{Database, Source};
use std::{collections::BTreeSet, sync::Arc};

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
    let (path, digest, bytes) = physical_file(&resource.body_path)?;
    if digest != resource.bytes_sha256 {
        return Err("font resource bytes do not match the captured SHA-256".to_string());
    }
    if !paths.insert(path.clone()) {
        return Err("duplicate physical font resource path".to_string());
    }
    /* WHY: digestを検証したbytesを登録し、同じパスを開き直して別の世代を取り込まない。 */
    database.load_font_source(Source::SharedFile(path, Arc::new(bytes)));
    Ok(())
}
