use super::{ProbeNames, ProbeOptions, ProbeResult};
use image::ImageFormat;
use katana_render_runtime::HtmlBrowserSession;

pub(super) fn write_frame(
    options: &ProbeOptions,
    session: &mut HtmlBrowserSession,
    scroll_y: f32,
) -> ProbeResult<()> {
    let (frame, metrics) = session
        .latest_frame_with_metrics()
        .ok_or("HTML runtime did not produce a frame with scroll metrics")?;
    /* WHY: 検証位置はCSS px、利用側の入力差分はphysical pxで渡す。 */
    let delta_y = (scroll_y.min(metrics.max_scroll_y()) - metrics.scroll_y)
        * frame.viewport.device_scale_factor;
    if delta_y != 0.0 {
        session.dispatch_input(katana_render_runtime::HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y,
        })?;
    }
    let frame = session
        .latest_frame()
        .ok_or("HTML runtime did not produce a frame")?;
    let path = frame_path(options, scroll_y);
    save_png(&path, frame)?;
    println!(
        "png={} width={} height={} scale={} scroll_y={} generation={}",
        path.display(),
        frame.viewport.width,
        frame.viewport.height,
        frame.viewport.device_scale_factor,
        frame.scroll_y,
        frame.generation
    );
    Ok(())
}

fn frame_path(options: &ProbeOptions, scroll_y: f32) -> std::path::PathBuf {
    options.output.join(format!(
        "{}x{}-scale-{}-scroll-{}.png",
        options.width,
        options.height,
        ProbeNames::scale_name(options.scale),
        ProbeNames::scroll_name(scroll_y)
    ))
}

fn save_png(
    path: &std::path::Path,
    frame: &katana_render_runtime::HtmlBrowserFrame,
) -> ProbeResult<()> {
    image::save_buffer_with_format(
        path,
        &frame.pixels,
        frame.viewport.width,
        frame.viewport.height,
        image::ColorType::Rgba8,
        ImageFormat::Png,
    )?;
    Ok(())
}
