use super::{FallbackAttributes, fallback_face_attribute_score, usvg};

pub(super) fn scan_fallback_faces(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
    attributes: FallbackAttributes,
    mut probe: impl FnMut(usvg::fontdb::ID, char) -> Option<bool>,
) -> (Option<usvg::fontdb::ID>, bool) {
    let mut best = None;
    let mut cacheable = true;
    for face in database.faces().filter(|face| face.id != base_face_id) {
        let score = score_face(face, attributes);
        /* WHY: 同点では先のfaceを保ち、選択結果を改善しないfontのfile解析を省く。 */
        if best.is_some_and(|(best_score, _)| score >= best_score) {
            continue;
        }
        let support = probe(face.id, character);
        cacheable &= support.is_some();
        if support == Some(true) {
            /* WHY: 全属性一致の最小scoreより良い候補はないため、後続faceの走査を省く。 */
            if score == (0, 0, 0, 0) {
                return (Some(face.id), cacheable);
            }
            best = Some((score, face.id));
        }
    }
    (best.map(|(_, face_id)| face_id), cacheable)
}

fn score_face(face: &usvg::fontdb::FaceInfo, attributes: FallbackAttributes) -> (u8, u8, u8, u16) {
    fallback_face_attribute_score(
        face,
        attributes.style,
        usvg::fontdb::Weight(attributes.weight),
        attributes.stretch,
    )
}
