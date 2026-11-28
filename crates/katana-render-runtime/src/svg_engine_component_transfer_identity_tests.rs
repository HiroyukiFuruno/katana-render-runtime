use super::{TransferFunction, is_dummy, transfer};

#[test]
fn identity_transfer_preserves_every_u8_channel_value() {
    let identity = TransferFunction::Identity;
    assert!(is_dummy(&identity));
    for value in u8::MIN..=u8::MAX {
        assert_eq!(transfer(&identity, value), value, "value={value}");
    }
}
