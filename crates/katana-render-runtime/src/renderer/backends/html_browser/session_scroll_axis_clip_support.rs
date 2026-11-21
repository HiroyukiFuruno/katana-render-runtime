pub(super) const VIEWPORT_WIDTH: f32 = 320.0;
pub(super) const VIEWPORT_HEIGHT: f32 = 240.0;
pub(super) const SOURCE_ORIGIN: &str = "https://example.test/scroll-axis-clip.html";
pub(super) const RED: [u8; 4] = [255, 0, 0, 255];
pub(super) const WHITE: [u8; 4] = [255, 255, 255, 255];
pub(super) const BLUE: [u8; 4] = [0, 0, 255, 255];
pub(super) const LIME: [u8; 4] = [0, 255, 0, 255];
pub(super) const MAGENTA: [u8; 4] = [255, 0, 255, 255];

pub(super) const HTML_TEMPLATE: &str = r#"<!doctype html><html><head><style>
html,body{margin:0;width:100%;height:100%;background:white}
#parent{position:absolute;left:0;top:0;width:__PARENT_WIDTH__px;height:__PARENT_HEIGHT__px;overflow-x:__OVERFLOW_X__;overflow-y:__OVERFLOW_Y__;border-radius:12px;background:white}
#child{position:absolute;left:__CHILD_LEFT__px;top:__CHILD_TOP__px;width:__CHILD_WIDTH__px;height:__CHILD_HEIGHT__px;background:red}
.observer{position:absolute;width:10px;height:10px;background:blue}
#ratio-state{position:fixed;left:5px;bottom:5px;width:10px;height:10px;background:blue}
#active-state{position:fixed;left:5px;top:5px;width:10px;height:10px;background:blue}
#clipped-state{position:fixed;left:20px;top:5px;width:10px;height:10px;background:blue}
#click-state{position:fixed;right:5px;bottom:5px;width:10px;height:10px;background:blue}
#observer-ratio-debug{position:fixed;right:5px;top:5px;width:1px;height:1px;background:blue}
#observer-intersection-debug{position:fixed;right:7px;top:5px;width:1px;height:1px;background:blue}
#observer-root-debug{position:fixed;right:9px;top:5px;width:1px;height:1px;background:blue}
</style></head><body><div id="parent"><div id="child">
<div id="active" class="observer" style="left:__ACTIVE_LEFT__px;top:__ACTIVE_TOP__px"></div>
<div id="clipped" class="observer" style="left:__CLIPPED_LEFT__px;top:__CLIPPED_TOP__px"></div>
<div id="blocked-click" style="position:absolute;left:__CLICK_LEFT__px;top:__CLICK_TOP__px;width:20px;height:20px;background:blue" onclick="document.getElementById('click-state').style.background='red'"></div>
</div></div><div id="ratio-state"></div><div id="active-state"></div><div id="clipped-state"></div><div id="click-state"></div><div id="observer-ratio-debug"></div><div id="observer-intersection-debug"></div><div id="observer-root-debug"></div><script>
const child=document.getElementById('child');
const expectedRatio=__EXPECTED_INTERSECTION_RATIO__;
const observer=new IntersectionObserver(entries=>{for(const entry of entries){
if(entry.target===child){
const ratioDebug=document.getElementById('observer-ratio-debug');
const intersectionDebug=document.getElementById('observer-intersection-debug');
const rootDebug=document.getElementById('observer-root-debug');
const root=entry.rootBounds||{};
ratioDebug.style.background=`rgb(${Math.round(entry.intersectionRatio*255)},${entry.isIntersecting?255:0},0)`;
intersectionDebug.style.background=`rgb(${Math.min(255,Math.round(entry.intersectionRect.width))},${Math.min(255,Math.round(entry.intersectionRect.height))},0)`;
rootDebug.style.background=`rgb(${Math.min(255,Math.round(root.width||0))},${Math.min(255,Math.round(root.height||0))},0)`;
document.getElementById('ratio-state').style.background=entry.isIntersecting&&Math.abs(entry.intersectionRatio-expectedRatio)<=0.01?'lime':'magenta';}
else{const id=entry.target.id==='active'?'active-state':'clipped-state';document.getElementById(id).style.background=entry.isIntersecting?'lime':'magenta';}
}}, {threshold:[0,0.1,0.12,0.16,1]});
observer.observe(child);
observer.observe(document.getElementById('active'));
observer.observe(document.getElementById('clipped'));
</script></body></html>"#;

pub(super) struct AxisCase {
    pub(super) name: &'static str,
    pub(super) parent_width: u32,
    pub(super) parent_height: u32,
    pub(super) child_left: i32,
    pub(super) child_top: i32,
    pub(super) child_width: u32,
    pub(super) child_height: u32,
    pub(super) overflow_x: &'static str,
    pub(super) overflow_y: &'static str,
    pub(super) active_left: u32,
    pub(super) active_top: u32,
    pub(super) clipped_left: u32,
    pub(super) clipped_top: u32,
    pub(super) click_left: u32,
    pub(super) click_top: u32,
    pub(super) visible_pixel: (u32, u32),
    pub(super) clipped_pixel: (u32, u32),
    pub(super) expected_content_width: f32,
    pub(super) expected_content_height: f32,
    pub(super) visible_axis_ratio: f32,
    pub(super) expected_intersection_ratio: f32,
}

pub(super) const CASES: [AxisCase; 2] = [
    AxisCase {
        name: "horizontal clip leaves vertical axis visible",
        parent_width: 100,
        parent_height: 40,
        child_left: 0,
        child_top: 0,
        child_width: 500,
        child_height: 400,
        overflow_x: "clip",
        overflow_y: "visible",
        active_left: 20,
        active_top: 60,
        clipped_left: 130,
        clipped_top: 5,
        click_left: 130,
        click_top: 10,
        visible_pixel: (2, 42),
        clipped_pixel: (105, 20),
        expected_content_width: VIEWPORT_WIDTH,
        expected_content_height: 400.0,
        visible_axis_ratio: 0.6,
        expected_intersection_ratio: 0.12,
    },
    AxisCase {
        name: "vertical clip leaves horizontal axis visible",
        parent_width: 40,
        parent_height: 100,
        child_left: 0,
        child_top: 0,
        child_width: 500,
        child_height: 400,
        overflow_x: "visible",
        overflow_y: "clip",
        active_left: 60,
        active_top: 20,
        clipped_left: 5,
        clipped_top: 120,
        click_left: 10,
        click_top: 130,
        visible_pixel: (42, 2),
        clipped_pixel: (20, 105),
        expected_content_width: 500.0,
        expected_content_height: VIEWPORT_HEIGHT,
        visible_axis_ratio: 0.64,
        expected_intersection_ratio: 0.16,
    },
];
