pub(crate) struct ScriptCompiler;

impl ScriptCompiler {
    pub(crate) fn compile_with_origin<'scope>(
        scope: &mut v8::PinScope<'scope, '_>,
        source: v8::Local<'scope, v8::String>,
        origin_name: v8::Local<'scope, v8::String>,
    ) -> Option<v8::Local<'scope, v8::Script>> {
        let origin = v8::ScriptOrigin::new(
            scope,
            origin_name.into(),
            0,
            0,
            false,
            0,
            Some(origin_name.into()),
            false,
            false,
            false,
            None,
        );
        v8::Script::compile(scope, source, Some(&origin))
    }
}
