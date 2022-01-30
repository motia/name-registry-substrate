use sp_runtime::traits::Hash;
use crate::{mock::*, Error, NameEntry};
use frame_support::{assert_noop, assert_ok};

#[test]
fn can_register() {
	new_test_ext().execute_with(|| {
		let name = vec![48, 62, 77, 66, 21, 55, 121, 55];
		assert_ok!(TemplateModule::register(
			Origin::signed(200),
			name.clone(),
			40
		));

		let name_hash = TemplateModule::name_id(&name);

		let name_entry = TemplateModule::name_entries(name_hash);

		assert_eq!(name_entry, Some(NameEntry{
			owner: 200,
			name: name.clone(),
			expires_at: 41
		}));

		assert_eq!(TemplateModule::name_cnt(), 1);

		let owned = TemplateModule::name_entries_owned(200);
		assert_eq!(owned.len(), 1);
		assert_eq!(owned[0], name_hash);

		assert_eq!(TemplateModule::is_name_owner(&name_hash, &200).unwrap_or(false), true);

		System::assert_last_event(Event::TemplateModule(crate::Event::<Test>::Registered(200, name.clone(), 41)));
	});
}

#[test]
fn can_renew() {
	new_test_ext().execute_with(|| {
		let name = vec![48, 62, 77, 66, 21, 55, 121, 55];
		assert_ok!(TemplateModule::register(
			Origin::signed(200),
			name.clone(),
			40
		));
		
		assert_ok!(TemplateModule::renew(
			Origin::signed(200),
			name.clone(),
			2
		));
		System::assert_last_event(Event::TemplateModule(crate::Event::<Test>::Renewed(200, name.clone(), 43)));
	});
}

#[test]
fn can_cancel() {
	new_test_ext().execute_with(|| {
		let name = vec![48, 62, 77, 66, 21, 55, 121, 55];
		assert_ok!(TemplateModule::register(
			Origin::signed(200),
			name.clone(),
			40
		));
		
		assert_ok!(TemplateModule::cancel(
			Origin::signed(200),
			name.clone(),
		));
		System::assert_last_event(Event::TemplateModule(crate::Event::<Test>::Canceled(200, name.clone(), 1)));
	});
}
