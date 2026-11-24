use objc2_core_foundation::{CFArray, CFDictionary, CFRetained, CFSet, CFString, CFURL};
use objc2_core_text::{
    CTFont, CTFontDescriptor, CTFontSymbolicTraits, CTFontUIFontType, kCTFontFamilyNameAttribute,
    kCTFontNameAttribute, kCTFontURLAttribute,
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

const SYSTEM_POINT_SIZE: f64 = 12.0;
const REGULAR_NAME: &str = ".SFNS-Regular";
const ITALIC_NAME: &str = ".SFNS-RegularItalic";
const REGULAR_PATH: &str = "/System/Library/Fonts/SFNS.ttf";
const ITALIC_PATH: &str = "/System/Library/Fonts/SFNSItalic.ttf";

pub(super) fn resolve_paths<'a>(
    families: &BTreeSet<&'a str>,
    scan: impl FnOnce(&BTreeSet<&'a str>) -> BTreeSet<PathBuf>,
) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    for &family in families {
        let direct = family_paths(family);
        if direct.is_empty() {
            unresolved.insert(family);
        }
        paths.extend(direct);
    }
    if !unresolved.is_empty() {
        paths.extend(scan(&unresolved));
    }
    paths
}

fn family_paths(family: &str) -> BTreeSet<PathBuf> {
    /* WHY: 既知の標準書体では2ファイルを直接取得し、全バリエーションの列挙を省く。 */
    if family == "system font"
        && let Some(paths) = system_ui_paths()
    {
        return paths;
    }
    registered_family_paths(family)
}

fn system_ui_paths() -> Option<BTreeSet<PathBuf>> {
    let regular = system_ui_font()?;
    /* SAFETY: null transformはCoreTextが文書化するidentity transform。 */
    let italic = unsafe {
        regular.copy_with_symbolic_traits(
            SYSTEM_POINT_SIZE,
            std::ptr::null(),
            CTFontSymbolicTraits::ItalicTrait,
            CTFontSymbolicTraits::ItalicTrait,
        )
    }?;
    [
        system_ui_font_path(&regular, REGULAR_NAME, Path::new(REGULAR_PATH), false),
        system_ui_font_path(&italic, ITALIC_NAME, Path::new(ITALIC_PATH), true),
    ]
    .into_iter()
    .collect()
}

fn system_ui_font() -> Option<CFRetained<CTFont>> {
    /* SAFETY: CoreTextへ公開UI種別、正のサイズ、language未指定を渡す。 */
    unsafe { CTFont::new_ui_font_for_language(CTFontUIFontType::System, SYSTEM_POINT_SIZE, None) }
}

fn system_ui_font_path(
    font: &CTFont,
    expected_name: &str,
    expected_path: &Path,
    italic: bool,
) -> Option<PathBuf> {
    /* SAFETY: CoreText fontから公開descriptorを取得する。 */
    let descriptor = unsafe { font.font_descriptor() };
    /* SAFETY: name属性をCFStringへ型検査し、system UI生成名だけを受理する。 */
    let postscript_name = unsafe { descriptor.attribute(kCTFontNameAttribute) }?
        .downcast::<CFString>()
        .ok()?
        .to_string();
    if postscript_name != expected_name {
        return None;
    }
    let path = descriptor_path(&descriptor)?;
    /* SAFETY: CoreTextが作成したfontから公開のスタイル属性を取得する。 */
    let traits = unsafe { font.symbolic_traits() };
    if traits.contains(CTFontSymbolicTraits::ItalicTrait) != italic {
        return None;
    }
    let canonical = path.canonicalize().ok()?;
    (path == expected_path && canonical == expected_path && path.is_file()).then_some(path)
}

fn registered_family_paths(family: &str) -> BTreeSet<PathBuf> {
    matching_descriptors(family)
        .iter()
        .flat_map(|descriptors| (0..descriptors.len()).filter_map(|index| descriptors.get(index)))
        .filter_map(|descriptor| descriptor_path(&descriptor))
        .filter(|path| path.is_file())
        .collect()
}

fn matching_descriptors(family: &str) -> Option<CFRetained<CFArray<CTFontDescriptor>>> {
    /* SAFETY: CoreTextの公開属性定数はプロセスの存続中有効。 */
    let key = unsafe { kCTFontFamilyNameAttribute };
    let name = CFString::from_str(family);
    let attributes = CFDictionary::from_slices(&[key], &[&*name]);
    /* SAFETY: family名は公開APIが要求するCFStringで渡す。 */
    let descriptor = unsafe { CTFontDescriptor::with_attributes((*attributes).as_ref()) };
    let mandatory = mandatory_family_key(key)?;
    /* SAFETY: 引数はCFStringの集合。戻り値はCTFontDescriptorの配列という公開API契約。 */
    unsafe {
        descriptor
            .matching_font_descriptors(Some(&mandatory))
            .map(|descriptors| CFRetained::cast_unchecked(descriptors))
    }
}

fn mandatory_family_key(key: &CFString) -> Option<CFRetained<CFSet>> {
    let mut keys = [std::ptr::from_ref(key).cast()];
    /* SAFETY: 要素は1件のCFStringで、Core Foundationの参照管理を使用する。 */
    unsafe {
        CFSet::new(
            None,
            keys.as_mut_ptr(),
            1,
            &objc2_core_foundation::kCFTypeSetCallBacks,
        )
    }
}

fn descriptor_path(descriptor: &CTFontDescriptor) -> Option<PathBuf> {
    /* SAFETY: CoreTextから取得した所有済み属性をCFURLへ型検査してから扱う。 */
    let url = unsafe { descriptor.attribute(kCTFontURLAttribute) }?;
    url.downcast::<CFURL>().ok()?.to_file_path()
}

#[cfg(test)]
mod tests {
    use super::{
        BTreeSet, Path, PathBuf, REGULAR_NAME, REGULAR_PATH, family_paths, registered_family_paths,
        resolve_paths, system_ui_font, system_ui_font_path, system_ui_paths,
    };

    #[test]
    fn coretext_resolves_registered_families_and_scans_only_unresolved_families() {
        let missing = "krr unregistered fallback test family";
        let paths = resolve_paths(&BTreeSet::from(["times", missing]), |unresolved| {
            assert_eq!(unresolved, &BTreeSet::from([missing]));
            BTreeSet::from([PathBuf::from("/tmp/krr-legacy-fallback.ttf")])
        });

        assert!(paths.contains(&PathBuf::from("/System/Library/Fonts/Times.ttc")));
        assert!(paths.contains(&PathBuf::from("/tmp/krr-legacy-fallback.ttf")));
    }

    #[test]
    fn system_font_fast_path_matches_registered_family_paths() -> Result<(), &'static str> {
        let registered = registered_family_paths("system font");
        let fast = system_ui_paths().ok_or("SFNS normal and italic files missing")?;
        assert!(!registered.is_empty());
        assert_eq!(fast, registered);
        assert_eq!(family_paths("system font"), fast);
        Ok(())
    }

    #[test]
    fn system_font_rejects_mismatched_face_style_and_file() -> Result<(), &'static str> {
        let font = system_ui_font().ok_or("CoreText system font missing")?;
        let path = Path::new(REGULAR_PATH);
        assert!(system_ui_font_path(&font, "Other Face", path, false).is_none());
        assert!(system_ui_font_path(&font, REGULAR_NAME, path, true).is_none());
        assert!(system_ui_font_path(&font, REGULAR_NAME, Path::new("other.ttf"), false).is_none());
        Ok(())
    }
}
