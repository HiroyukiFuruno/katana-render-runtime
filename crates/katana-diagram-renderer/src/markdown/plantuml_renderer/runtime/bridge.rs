use super::super::theme::PlantUmlRenderStyle;
use jni::{
    JavaVM, jni_sig, jni_str,
    objects::{JByteArray, JObject, JString, JValue},
};

pub(crate) struct PlantUmlJvmBridgeOps;

impl PlantUmlJvmBridgeOps {
    pub(crate) fn render(
        java_vm: &JavaVM,
        source: &str,
        style: &PlantUmlRenderStyle,
    ) -> Result<String, String> {
        let (svg, description) = java_vm
            .attach_current_thread(|env| -> jni::errors::Result<(String, String)> {
                let source = env.new_string(source)?;
                let reader = Self::source_string_reader(env, &source, style)?;
                let output_stream = Self::byte_array_output_stream(env)?;
                let description = Self::render_to_stream(env, &reader, &output_stream, style)?;
                let svg = Self::svg_from_stream(env, &output_stream)?;
                Ok((svg, description))
            })
            .map_err(|error| error.to_string())?;
        if description.to_ascii_lowercase().contains("error") {
            return Err(format!("PlantUML render failed: {description}"));
        }
        Ok(svg)
    }

    fn source_string_reader<'local>(
        env: &mut jni::Env<'local>,
        source: &JString<'local>,
        style: &PlantUmlRenderStyle,
    ) -> jni::errors::Result<JObject<'local>> {
        let defines = Self::empty_defines(env)?;
        let config = Self::config_list(env, style)?;
        env.new_object(
            jni_str!("net/sourceforge/plantuml/SourceStringReader"),
            jni_sig!(
                "(Lnet/sourceforge/plantuml/preproc/Defines;Ljava/lang/String;Ljava/util/List;)V"
            ),
            &[
                JValue::Object(&defines),
                JValue::Object(source),
                JValue::Object(&config),
            ],
        )
    }

    fn empty_defines<'local>(env: &mut jni::Env<'local>) -> jni::errors::Result<JObject<'local>> {
        env.call_static_method(
            jni_str!("net/sourceforge/plantuml/preproc/Defines"),
            jni_str!("createEmpty"),
            jni_sig!("()Lnet/sourceforge/plantuml/preproc/Defines;"),
            &[],
        )?
        .l()
    }

    fn config_list<'local>(
        env: &mut jni::Env<'local>,
        style: &PlantUmlRenderStyle,
    ) -> jni::errors::Result<JObject<'local>> {
        let list = env.new_object(jni_str!("java/util/ArrayList"), jni_sig!("()V"), &[])?;
        for line in style.config_lines() {
            let java_line = env.new_string(line)?;
            env.call_method(
                &list,
                jni_str!("add"),
                jni_sig!("(Ljava/lang/Object;)Z"),
                &[JValue::Object(&java_line)],
            )?;
        }
        Ok(list)
    }

    fn byte_array_output_stream<'local>(
        env: &mut jni::Env<'local>,
    ) -> jni::errors::Result<JObject<'local>> {
        env.new_object(
            jni_str!("java/io/ByteArrayOutputStream"),
            jni_sig!("()V"),
            &[],
        )
    }

    fn render_to_stream<'local>(
        env: &mut jni::Env<'local>,
        reader: &JObject<'local>,
        output_stream: &JObject<'local>,
        style: &PlantUmlRenderStyle,
    ) -> jni::errors::Result<String> {
        let format_option = Self::svg_format_option(env, style)?;
        let description = Self::call_output_image(env, reader, output_stream, &format_option)?;
        if description.is_null() {
            return Ok("error: PlantUML did not return a diagram description".to_string());
        }
        Self::description(env, description)
    }

    fn svg_format_option<'local>(
        env: &mut jni::Env<'local>,
        style: &PlantUmlRenderStyle,
    ) -> jni::errors::Result<JObject<'local>> {
        let svg_format = env
            .get_static_field(
                jni_str!("net/sourceforge/plantuml/FileFormat"),
                jni_str!("SVG"),
                jni_sig!("Lnet/sourceforge/plantuml/FileFormat;"),
            )?
            .l()?;
        let format_option = env.new_object(
            jni_str!("net/sourceforge/plantuml/FileFormatOption"),
            jni_sig!("(Lnet/sourceforge/plantuml/FileFormat;)V"),
            &[JValue::Object(&svg_format)],
        )?;
        if style.dark_mode() {
            return Self::dark_format_option(env, &format_option);
        }
        Ok(format_option)
    }

    fn dark_format_option<'local>(
        env: &mut jni::Env<'local>,
        format_option: &JObject<'local>,
    ) -> jni::errors::Result<JObject<'local>> {
        let dark_mapper = env
            .get_static_field(
                jni_str!("net/sourceforge/plantuml/klimt/color/ColorMapper"),
                jni_str!("DARK_MODE"),
                jni_sig!("Lnet/sourceforge/plantuml/klimt/color/ColorMapper;"),
            )?
            .l()?;
        env.call_method(
            format_option,
            jni_str!("withColorMapper"),
            jni_sig!(
                "(Lnet/sourceforge/plantuml/klimt/color/ColorMapper;)Lnet/sourceforge/plantuml/FileFormatOption;"
            ),
            &[JValue::Object(&dark_mapper)],
        )?
        .l()
    }

    fn call_output_image<'local>(
        env: &mut jni::Env<'local>,
        reader: &JObject<'local>,
        output_stream: &JObject<'local>,
        format_option: &JObject<'local>,
    ) -> jni::errors::Result<JObject<'local>> {
        env.call_method(
            reader,
            jni_str!("outputImage"),
            jni_sig!(
                "(Ljava/io/OutputStream;Lnet/sourceforge/plantuml/FileFormatOption;)Lnet/sourceforge/plantuml/core/DiagramDescription;"
            ),
            &[JValue::Object(output_stream), JValue::Object(format_option)],
        )?
        .l()
    }

    fn description<'local>(
        env: &mut jni::Env<'local>,
        description: JObject<'local>,
    ) -> jni::errors::Result<String> {
        let text = env
            .call_method(
                description,
                jni_str!("getDescription"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            )?
            .l()?;
        let java_text = env.cast_local::<JString>(text)?;
        java_text.try_to_string(env)
    }

    fn svg_from_stream(
        env: &mut jni::Env<'_>,
        output_stream: &JObject<'_>,
    ) -> jni::errors::Result<String> {
        let bytes_object = env
            .call_method(
                output_stream,
                jni_str!("toByteArray"),
                jni_sig!("()[B"),
                &[],
            )?
            .l()?;
        let bytes = env.cast_local::<JByteArray>(bytes_object)?;
        let svg_bytes = env.convert_byte_array(&bytes)?;
        Ok(String::from_utf8_lossy(&svg_bytes).to_string())
    }
}
