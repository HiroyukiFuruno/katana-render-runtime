mod bridge;

use super::resolve::PlantUmlRuntimePaths;
use super::theme::PlantUmlRenderStyle;
use bridge::PlantUmlJvmBridgeOps;
use jni::{InitArgsBuilder, JNIVersion, JavaVM};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

static PLANTUML_JVM: Mutex<Option<PlantUmlJvm>> = Mutex::new(None);

pub(crate) struct PlantUmlJvmRuntimeOps;

struct PlantUmlJvm {
    java_vm: JavaVM,
    jar_path: PathBuf,
}

impl PlantUmlJvmRuntimeOps {
    pub(crate) fn render_svg(
        source: &str,
        paths: &PlantUmlRuntimePaths,
        style: &PlantUmlRenderStyle,
    ) -> Result<String, String> {
        let mut guard = Self::jvm_guard();
        if guard.is_none() {
            *guard = Some(Self::create_jvm(paths)?);
        }
        let jvm = guard
            .as_ref()
            .ok_or_else(|| "PlantUML JVM is not initialized".to_string())?;
        if jvm.jar_path != paths.jar_path {
            return Err("PlantUML JVM is already initialized with another JAR".to_string());
        }
        PlantUmlJvmBridgeOps::render(&jvm.java_vm, source, style)
    }

    fn create_jvm(paths: &PlantUmlRuntimePaths) -> Result<PlantUmlJvm, String> {
        let args = Self::jvm_args(&paths.jar_path)?;
        let java_vm = JavaVM::with_libjvm(args, || Ok(paths.jvm_path.clone()))
            .map_err(|error| error.to_string())?;
        Ok(PlantUmlJvm {
            java_vm,
            jar_path: paths.jar_path.clone(),
        })
    }

    fn jvm_args(jar_path: &std::path::Path) -> Result<jni::InitArgs<'_>, String> {
        let class_path = format!("-Djava.class.path={}", jar_path.display());
        InitArgsBuilder::new()
            .version(JNIVersion::V1_8)
            .option(class_path)
            .option("-Djava.awt.headless=true")
            .build()
            .map_err(|error| error.to_string())
    }

    fn jvm_guard() -> MutexGuard<'static, Option<PlantUmlJvm>> {
        match PLANTUML_JVM.lock() {
            Ok(guard) => guard,
            Err(error) => error.into_inner(),
        }
    }
}
