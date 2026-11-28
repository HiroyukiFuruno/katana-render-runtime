use super::{Data, Opaque};

/// The superclass of object and function templates.
#[repr(C)]
#[derive(Debug)]
pub struct Template(Opaque);

impl_deref! { Data for Template }
impl_try_from! { Data for Template if v => v.is_function_template() || v.is_object_template() }
impl_from! { FunctionTemplate for Template }
impl_from! { ObjectTemplate for Template }
impl_eq! { for Template }
impl_partial_eq! { Data for Template use identity }
impl_partial_eq! { Template for Template use identity }
impl_partial_eq! { FunctionTemplate for Template use identity }
impl_partial_eq! { ObjectTemplate for Template use identity }

/// A FunctionTemplate is used to create functions at runtime. There
/// can only be one function created from a FunctionTemplate in a
/// context. The lifetime of the created function is equal to the
/// lifetime of the context. So in case the embedder needs to create
/// temporary functions that can be collected using Scripts is
/// preferred.
///
/// Any modification of a FunctionTemplate after first instantiation will trigger
/// a crash.
///
/// A FunctionTemplate can have properties, these properties are added to the
/// function object when it is created.
///
/// A FunctionTemplate has a corresponding instance template which is
/// used to create object instances when the function is used as a
/// constructor. Properties added to the instance template are added to
/// each object instance.
///
/// A FunctionTemplate can have a prototype template. The prototype template
/// is used to create the prototype object of the function.
///
/// The following example shows how to use a FunctionTemplate:
///
/// ```ignore
///    v8::Local<v8::FunctionTemplate> t = v8::FunctionTemplate::New(isolate);
///    t->Set(isolate, "func_property", v8::Number::New(isolate, 1));
///
///    v8::Local<v8::Template> proto_t = t->PrototypeTemplate();
///    proto_t->Set(isolate,
///                 "proto_method",
///                 v8::FunctionTemplate::New(isolate, InvokeCallback));
///    proto_t->Set(isolate, "proto_const", v8::Number::New(isolate, 2));
///
///    v8::Local<v8::ObjectTemplate> instance_t = t->InstanceTemplate();
///    instance_t->SetAccessor(
///        String::NewFromUtf8Literal(isolate, "instance_accessor"),
///        InstanceAccessorCallback);
///    instance_t->SetHandler(
///        NamedPropertyHandlerConfiguration(PropertyHandlerCallback));
///    instance_t->Set(String::NewFromUtf8Literal(isolate, "instance_property"),
///                    Number::New(isolate, 3));
///
///    v8::Local<v8::Function> function = t->GetFunction();
///    v8::Local<v8::Object> instance = function->NewInstance();
/// ```
///
/// Let's use "function" as the JS variable name of the function object
/// and "instance" for the instance object created above. The function
/// and the instance will have the following properties:
///
/// ```ignore
///   func_property in function == true;
///   function.func_property == 1;
///
///   function.prototype.proto_method() invokes 'InvokeCallback'
///   function.prototype.proto_const == 2;
///
///   instance instanceof function == true;
///   instance.instance_accessor calls 'InstanceAccessorCallback'
///   instance.instance_property == 3;
/// ```
///
/// A FunctionTemplate can inherit from another one by calling the
/// FunctionTemplate::Inherit method. The following graph illustrates
/// the semantics of inheritance:
///
/// ```ignore
///   FunctionTemplate Parent  -> Parent() . prototype -> { }
///     ^                                                  ^
///     | Inherit(Parent)                                  | .__proto__
///     |                                                  |
///   FunctionTemplate Child   -> Child()  . prototype -> { }
/// ```
///
/// A FunctionTemplate 'Child' inherits from 'Parent', the prototype
/// object of the Child() function has __proto__ pointing to the
/// Parent() function's prototype object. An instance of the Child
/// function has all properties on Parent's instance templates.
///
/// Let Parent be the FunctionTemplate initialized in the previous
/// section and create a Child FunctionTemplate by:
///
/// ```ignore
///   Local<FunctionTemplate> parent = t;
///   Local<FunctionTemplate> child = FunctionTemplate::New();
///   child->Inherit(parent);
///
///   Local<Function> child_function = child->GetFunction();
///   Local<Object> child_instance = child_function->NewInstance();
/// ```
///
/// The Child function and Child instance will have the following
/// properties:
///
/// ```ignore
///   child_func.prototype.__proto__ == function.prototype;
///   child_instance.instance_accessor calls 'InstanceAccessorCallback'
///   child_instance.instance_property == 3;
/// ```
///
/// The additional 'c_function' parameter refers to a fast API call, which
/// must not trigger GC or JavaScript execution, or call into V8 in other
/// ways. For more information how to define them, see
/// include/v8-fast-api-calls.h. Please note that this feature is still
/// experimental.
#[repr(C)]
#[derive(Debug)]
pub struct FunctionTemplate(Opaque);

impl_deref! { Template for FunctionTemplate }
impl_try_from! { Data for FunctionTemplate if v => v.is_function_template() }
impl_try_from! { Template for FunctionTemplate if v => v.is_function_template() }
impl_eq! { for FunctionTemplate }
impl_partial_eq! { Data for FunctionTemplate use identity }
impl_partial_eq! { Template for FunctionTemplate use identity }
impl_partial_eq! { FunctionTemplate for FunctionTemplate use identity }

/// An ObjectTemplate is used to create objects at runtime.
///
/// Properties added to an ObjectTemplate are added to each object
/// created from the ObjectTemplate.
#[repr(C)]
#[derive(Debug)]
pub struct ObjectTemplate(Opaque);

impl_deref! { Template for ObjectTemplate }
impl_try_from! { Data for ObjectTemplate if v => v.is_object_template() }
impl_try_from! { Template for ObjectTemplate if v => v.is_object_template() }
impl_eq! { for ObjectTemplate }
impl_partial_eq! { Data for ObjectTemplate use identity }
impl_partial_eq! { Template for ObjectTemplate use identity }
impl_partial_eq! { ObjectTemplate for ObjectTemplate use identity }

/// A compiled JavaScript module, not yet tied to a Context.
#[repr(C)]
#[derive(Debug)]
pub struct UnboundModuleScript(Opaque);

impl_deref! { Data for UnboundModuleScript }
impl_eq! { for UnboundModuleScript }
impl_partial_eq! { Data for UnboundModuleScript use identity }
impl_partial_eq! { UnboundModuleScript for UnboundModuleScript use identity }

/// A compiled JavaScript script, not yet tied to a Context.
#[repr(C)]
#[derive(Debug)]
pub struct UnboundScript(Opaque);

impl_eq! { for UnboundScript }
impl_partial_eq! { UnboundScript for UnboundScript use identity }
