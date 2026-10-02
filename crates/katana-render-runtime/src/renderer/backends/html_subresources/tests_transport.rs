use super::super::HtmlSubresourceLoader;
use super::support::{
    LocalFixture, TestResult, assert_frame_contains, delayed_dynamic_server, local_resource_server,
    must_result, must_source, to_string, viewport,
};
use crate::renderer::backends::{HtmlBrowserSource, HtmlRuntime};

#[test]
fn local_resources_feed_css_v8_and_image_layout() -> TestResult {
    let fixture = LocalFixture::new()?;
    let source = HtmlBrowserSource::new(fixture.html(), fixture.origin()?).map_err(to_string)?;
    let loader = HtmlSubresourceLoader::new(&source);
    assert_eq!(loader.document_origin(), source.origin.as_str());
    let mut document =
        crate::renderer::backends::html_document::HtmlDocument::parse(&source.raw_html);
    let resources = loader.load(&mut document);
    let frame = HtmlRuntime
        .open(source, viewport()?)
        .map_err(to_string)?
        .latest_frame()
        .cloned()
        .ok_or("frame is missing")?;

    assert!(resources.stylesheets.contains_key("style.css"));
    assert!(
        resources
            .scripts
            .iter()
            .any(|script| script.source().contains("scripted"))
    );
    assert!(document.render().contains("data:image/svg+xml;base64,"));
    assert_frame_contains(&frame.pixels, [16, 185, 129]);
    assert_frame_contains(&frame.pixels, [239, 68, 68]);
    assert_frame_contains(&frame.pixels, [49, 130, 206]);
    Ok(())
}

#[test]
fn interactive_scripts_continue_after_an_ordinary_javascript_exception() -> TestResult {
    let fixture = LocalFixture::new()?;
    let source = must_source(
        &fixture,
        "<script>throw new Error('first script failed')</script>\
         <div id=continued style='width:40px;height:40px'></div>\
         <script>document.getElementById('continued').style.backgroundColor='#22c55e'</script>",
    );
    let frame = HtmlRuntime
        .open(source, viewport()?)
        .map_err(to_string)?
        .latest_frame()
        .cloned()
        .ok_or("frame is missing")?;

    assert_frame_contains(&frame.pixels, [34, 197, 94]);
    Ok(())
}

#[test]
fn same_origin_xhr_and_session_storage_drive_a_dynamic_frame() -> TestResult {
    let fixture = LocalFixture::new()?;
    std::fs::write(fixture.root.join("state.txt"), "ready").map_err(to_string)?;
    let source = must_source(
        &fixture,
        "<div id=dynamic style='width:40px;height:40px'></div><script>\
         if (NodeList !== Array || document.location.href !== location.href) throw new Error('browser globals');\
         localStorage.state='stored';\
         const xhr=new XMLHttpRequest();\
         xhr.open('GET','state.txt',true);\
         xhr.send();\
         xhr.onload=()=>{\
           if(xhr.status===200 && xhr.responseText==='ready' && localStorage.getItem('state')==='stored')\
             document.getElementById('dynamic').style.backgroundColor='#0ea5e9';\
         };\
         </script>",
    );
    let frame = HtmlRuntime
        .open(source, viewport()?)
        .map_err(to_string)?
        .latest_frame()
        .cloned()
        .ok_or("frame is missing")?;

    assert_frame_contains(&frame.pixels, [14, 165, 233]);
    Ok(())
}

#[test]
fn xhr_network_wait_is_excluded_from_the_javascript_execution_budget() -> TestResult {
    let (origin, server) = delayed_dynamic_server()?;
    let source = HtmlBrowserSource::new(
        "<div id=dynamic style='width:40px;height:40px'></div><script>\
         const xhr=new XMLHttpRequest();\
         xhr.open('GET','state.txt',true);\
         xhr.send();\
         xhr.onload=()=>{\
           if(xhr.status===200 && xhr.responseText==='ready')\
             document.getElementById('dynamic').style.backgroundColor='#a855f7';\
         };\
         </script>",
        format!("{origin}/index.html"),
    )
    .map_err(to_string)?;
    let frame = HtmlRuntime
        .open(source, viewport()?)
        .map_err(to_string)?
        .latest_frame()
        .cloned()
        .ok_or("frame is missing")?;
    let joined = must_result(server.join());

    assert!(joined.is_ok());
    assert_frame_contains(&frame.pixels, [168, 85, 247]);
    Ok(())
}

#[test]
fn blocked_xhr_requests_dispatch_errors_without_stopping_the_frame() -> TestResult {
    let fixture = LocalFixture::new()?;
    let source = must_source(
        &fixture,
        "<div id=dynamic style='width:40px;height:40px'></div><script>\
         let blocked=0;\
         const crossOrigin=new XMLHttpRequest();\
         crossOrigin.open('GET','https://example.test/state.txt');\
         crossOrigin.send();\
         crossOrigin.onerror=()=>{blocked+=1};\
         const post=new XMLHttpRequest();\
         post.open('POST','state.txt');\
         post.send();\
         post.onerror=()=>{\
           blocked+=1;\
           if(blocked===2) document.getElementById('dynamic').style.backgroundColor='#f97316';\
         };\
         </script>",
    );
    let frame = HtmlRuntime
        .open(source, viewport()?)
        .map_err(to_string)?
        .latest_frame()
        .cloned()
        .ok_or("frame is missing")?;

    assert_frame_contains(&frame.pixels, [249, 115, 22]);
    Ok(())
}

#[test]
fn html_source_rejects_invalid_origin_for_viewer_flow() {
    let invalid = HtmlBrowserSource::new("<div id=dynamic>Broken</div>", "http://[");
    assert!(invalid.is_err());
}

#[test]
fn local_document_source_rejects_invalid_navigation_in_transport_path() -> TestResult {
    let fixture = LocalFixture::new()?;
    let source = fixture.source("<div id=dynamic>Broken</div>")?;
    let runtime = HtmlRuntime.open(source.clone(), viewport()?);
    assert!(runtime.is_ok());
    assert!(HtmlBrowserSource::new("<div>broken</div>", "about:blank").is_err());
    Ok(())
}

#[test]
fn loopback_http_relative_css_script_and_image_requests_reach_the_runtime() -> TestResult {
    let (origin, requests, server) = local_resource_server()?;
    let source = HtmlBrowserSource::new(http_document(), format!("{origin}/index.html"))
        .map_err(to_string)?;
    let frame = HtmlRuntime
        .open(source, viewport()?)
        .map_err(to_string)?
        .latest_frame()
        .cloned()
        .ok_or("frame is missing")?;
    let joined = must_result(server.join());

    assert!(joined.is_ok());
    let requests = must_result(requests.lock());
    assert_eq!(*requests, ["/style.css", "/app.js", "/pixel.svg"]);
    assert_frame_contains(&frame.pixels, [16, 185, 129]);
    assert_frame_contains(&frame.pixels, [239, 68, 68]);
    assert_frame_contains(&frame.pixels, [49, 130, 206]);
    Ok(())
}

fn http_document() -> String {
    "<link rel=stylesheet href=style.css><div id=styled>Styled</div>\
         <div id=scripted style='width:80px;height:30px'>Scripted</div>\
         <script src=app.js></script>\
         <img src=pixel.svg style='width:40px;height:40px'>"
        .to_string()
}
