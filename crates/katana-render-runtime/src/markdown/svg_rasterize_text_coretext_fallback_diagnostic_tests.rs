use objc2_core_foundation::{CFRange, CFString};
use objc2_core_text::CTFont;

const DIAGNOSTIC_FONT_SIZE: f64 = 16.0;

#[test]
fn coretext_fallback_observes_actual_base_and_family() {
    for base in [None, Some("Helvetica"), Some("Helvetica-Bold")] {
        for character in ['注', 'A', '\u{10ffff}'] {
            observe_fallback(base, character);
        }
    }
}

fn observe_fallback(base_name: Option<&str>, character: char) {
    let Some(base_name) = base_name else {
        println!("CoreText fallback: base=None character={character:?} result=not_requested");
        return;
    };
    let name = CFString::from_str(base_name);
    /* WHY: CoreText が指定名の代替を返す場合もあるため、実際の base を記録する。 */
    let base = unsafe { CTFont::with_name(&name, DIAGNOSTIC_FONT_SIZE, std::ptr::null()) };
    let text = CFString::from_str(&character.to_string());
    let range = CFRange {
        location: 0,
        length: text.length(),
    };
    /* WHY: range は CFString 自身の UTF-16 長で、非 BMP 文字も範囲に収める。 */
    let fallback = unsafe { base.for_string(&text, range) };
    print_result(base_name, character, &base, &fallback);
}

fn print_result(requested: &str, character: char, base: &CTFont, fallback: &CTFont) {
    /* WHY: 同じ physical face と仮定せず、CoreText が返す名前をそのまま記録する。 */
    let (base_ps, base_family) = unsafe { (base.post_script_name(), base.family_name()) };
    let (fallback_ps, fallback_family) =
        unsafe { (fallback.post_script_name(), fallback.family_name()) };
    println!(
        "CoreText fallback: requested={requested:?} character={character:?} basePS={} baseFamily={} fallbackPS={} fallbackFamily={}",
        *base_ps, *base_family, *fallback_ps, *fallback_family
    );
}
