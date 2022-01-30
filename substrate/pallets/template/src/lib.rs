#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::inherent::Vec;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use frame_support::{
		sp_runtime::traits::{AccountIdConversion, SaturatedConversion,Hash},
		traits::{Currency, tokens::ExistenceRequirement},
		transactional,
		PalletId,
	};
	use scale_info::TypeInfo;

	#[cfg(feature = "std")]
	use frame_support::serde::{Deserialize, Serialize};

	type AccountOf<T> = <T as frame_system::Config>::AccountId;
	type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	// Struct for holding NameEntry information.
	#[derive(Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct NameEntry<T: Config> {
		pub name: Vec<u8>,
		pub expires_at: T::BlockNumber,
		pub owner: AccountOf<T>,
	}

	impl<T: Config> NameEntry<T> {
		pub fn eq_name(&self, b: Vec<u8>) -> bool {
			let a = &self.name;
			a.len() == b.len() && a.iter().zip(&b).all(|(a, b)| a == b)
		}
		
		pub fn eq_name_entry(&self, y: &Self) -> bool {
			self.expires_at == y.expires_at &&
			self.owner == y.owner &&
			self.eq_name(y.name.clone())
		}
	}

	impl<T: Config> PartialEq<NameEntry<T>> for NameEntry<T> {
		fn eq(&self, other: &Self) -> bool {
			Self::eq_name_entry(self, other)
		}
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types it depends on.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The Currency handler for the Names pallet.
		type Currency: Currency<Self::AccountId>;

		#[pallet::constant]
		type MaxNameOwned: Get<u32>;

		/// The treasury's pallet id, used for deriving its sovereign account ID.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// The maximum amount of Names a single account can own.
		#[pallet::constant]
		type BlockReservationCost: Get<u32>;

		/// The minimum length a name may be.
		#[pallet::constant]
		type MinLength: Get<u16>;

		/// The maximum length a name may be.
		#[pallet::constant]
		type MaxLength: Get<u16>;
	}

	// Errors.
	#[pallet::error]
	pub enum Error<T> {
		/// Handles arithmetic overflow when incrementing the Name counter.
		NameCntOverflow,
		/// Handles checking whether the NameEntry exists.
		NameNotExist,
		/// An account cannot own more NameEntry than `MaxNameOwned`.
		ExceedMaxNamesOwned,
		/// Handles checking that the NameEntry is owned by the account transferring.
		NotNameOwner,
		/// NameEntry expired
		NameExpired,
		/// NameEntry already registered
		NameAlreadyRegistered,
		/// Ensures that an account has enough funds to reserve a NameEntry.
		NotEnoughBalance,
		/// Ensures reservation is not 0.
		ZeroBlocksReserved,
		/// Ensures fee did not.
		FeeOverflow,
		/// A name is too short.
		NameTooShort,
		/// A name is too long.
		NameTooLong,
	}

	// Events.
	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new Name was successfully registered. \[sender, name_entry_id, expires_at\]
		Registered(T::AccountId, Vec<u8>, T::BlockNumber),
		/// Name price was successfully set. \[sender, name_entry_id, expires_at\]
		Renewed(T::AccountId, Vec<u8>, T::BlockNumber),
		/// Name price was successfully set. \[sender, name_entry_id, expires_at\]
		Canceled(T::AccountId, Vec<u8>, T::BlockNumber),
	}

	// Storage items.

	#[pallet::storage]
	#[pallet::getter(fn name_cnt)]
	/// Keeps track of the number of Names in existence.
	pub(super) type NameCnt<T: Config> = StorageValue<_, u64, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn name_entries)]
	/// Stores a Name's unique traits, owner and price.
	pub(super) type NameEntries<T: Config> = StorageMap<
		_,
		Twox64Concat, T::Hash,
		NameEntry<T>, OptionQuery
	>;

	#[pallet::storage]
	#[pallet::getter(fn name_entries_owned)]
	/// Keeps track of what accounts own what Name.
	pub(super) type NameEntriesOwned<T: Config> =
	StorageMap<
		_,
		Blake2_128Concat, T::AccountId,
		BoundedVec<T::Hash, T::MaxNameOwned>, ValueQuery
	>;

	// Our pallet's genesis configuration.
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub name_entries: Vec<(T::AccountId, Vec<u8>, u32)>,
	}

	// Required to implement default for GenesisConfig.
	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> GenesisConfig<T> {
			GenesisConfig { name_entries: vec![] }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			for (acct, name, num_blocks) in &self.name_entries {
				let _ = <Pallet<T>>::create_name(acct, name.clone(), *num_blocks);
			}
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new unique name.
		///
		/// The actual name creation is done in the `create_name()` function.
		#[pallet::weight(100)]
		pub fn register(
			origin: OriginFor<T>,
			name: Vec<u8>,
			num_blocks: u32,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			Self::ensure_name_len(&name)?;

			let expires_at = Self::create_name(&sender, name.clone(), num_blocks)?;

			// Logging to the console
			// TODO:
			// log::info!("A name is born with ID: {:?}.", name_entry_id);
			// Deposit our "Registered" event.
			Self::deposit_event(Event::Registered(sender, name, expires_at));
			Ok(())
		}

		/// Set the price for a Name.
		///
		/// Updates Name price and updates storage.
		#[pallet::weight(100)]
		#[transactional]
		pub fn renew(
			origin: OriginFor<T>,
			name: Vec<u8>,
			num_blocks: u32,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			Self::ensure_name_len(&name)?;
			let name_id = Self::name_id(&name);

			// Ensure the name exists and is called by the name owner
			ensure!(Self::is_name_owner(&name_id, &sender)?, <Error<T>>::NotNameOwner);

			let mut name_entry = Self::name_entries(&name_id).ok_or(<Error<T>>::NameNotExist)?;

			let current_block_number = <frame_system::Pallet<T>>::block_number();
			let new_reservation_start_block =
				if current_block_number > name_entry.expires_at { current_block_number }
					else { name_entry.expires_at };
			let new_expires_at = new_reservation_start_block + num_blocks.into();

			ensure!(new_expires_at > new_reservation_start_block, <Error<T>>::ZeroBlocksReserved);

			let fee = Self::calculate_fee(num_blocks)?;
			ensure!(T::Currency::free_balance(&sender) >= fee, <Error<T>>::NotEnoughBalance);

			name_entry.expires_at = new_expires_at;
			<NameEntries<T>>::insert(&name_id, name_entry);

			// pay for name registration
			T::Currency::transfer(&sender, &Self::account_id(), fee, ExistenceRequirement::KeepAlive)?;

			// Deposit a "Renewed" event.
			Self::deposit_event(Event::Renewed(sender, name, new_expires_at));

			Ok(())
		}

		/// Directly transfer a name to another recipient.
		///
		/// Any account that holds a name can send it to another Account. This will reset the asking
		/// price of the name, marking it not for sale.
		#[pallet::weight(100)]
		#[transactional]
		pub fn cancel(
			origin: OriginFor<T>,
			name: Vec<u8>,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;
			let name_id = T::Hashing::hash_of(&name);

			// Ensure the name exists and is called by the name owner
			ensure!(Self::is_name_owner(&name_id, &from)?, <Error<T>>::NotNameOwner);

			let mut name_entry = Self::name_entries(name_id).ok_or(<Error<T>>::NameNotExist)?;

			let current_block_number = <frame_system::Pallet<T>>::block_number();
			ensure!(name_entry.expires_at > current_block_number, <Error<T>>::NameExpired);

			let refunded_blocks = (name_entry.expires_at - current_block_number)
				// we are sure that refunded_blocks fills into u32
				.saturated_into::<u32>();

			ensure!(
				current_block_number + refunded_blocks.into() == name_entry.expires_at,
				<Error<T>>::NameExpired
			);


			let refund = Self::calculate_fee(
				refunded_blocks
			)?;

			ensure!(T::Currency::free_balance(&Self::account_id()) >= refund, <Error<T>>::NotEnoughBalance);

			name_entry.expires_at = current_block_number;
			<NameEntries<T>>::insert(name_id, name_entry);

			// refund for name cancel
			T::Currency::transfer(&Self::account_id(), &from, refund, ExistenceRequirement::KeepAlive)?;

			Self::deposit_event(Event::Canceled(from, name, current_block_number));

			Ok(())
		}
	}

	//** Our helper functions.**//

	impl<T: Config> Pallet<T> {
		pub fn ensure_name_len(name: &Vec<u8>) -> DispatchResult {
			let name_len = u16::try_from(name.len()).unwrap();
			ensure!(T::MinLength::get() < name_len, Error::<T>::NameTooShort);
			ensure!(T::MaxLength::get() > name_len, Error::<T>::NameTooLong);

			Ok(())
		}

		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account()
		}

		pub fn name_id(name: &Vec<u8>) -> T::Hash {
			T::Hashing::hash_of(name)
		}

		// Helper to create_name a Name.
		#[transactional]
		fn create_name(
			owner: &T::AccountId,
			name: Vec<u8>,
			num_blocks: u32,
		) -> Result<T::BlockNumber, DispatchError> {
			let current_block_number = <frame_system::Pallet<T>>::block_number();
			let eff_expires_at = current_block_number + num_blocks.into();

			ensure!(eff_expires_at > current_block_number, <Error<T>>::ZeroBlocksReserved);

			let name_id = Self::name_id(&name);

			// get old record, or init a new one
			let (name_entry, old_owner, already_registered) = match Self::name_entries(name_id) {
				Some(mut name_entry) => {
					ensure!(name_entry.expires_at < current_block_number, <Error<T>>::NameAlreadyRegistered);
					let old_owner = name_entry.owner.clone();
		
					name_entry.owner = owner.clone();
					name_entry.expires_at = eff_expires_at;

					(name_entry, old_owner, true)
				},
				None => {					
					(NameEntry {
						name: name.clone(),
						owner: owner.clone(),
						expires_at: eff_expires_at,
					}, owner.clone(), false)
				}
			};

			let fee = Self::calculate_fee(num_blocks)?;
			ensure!(T::Currency::free_balance(&owner) >= fee, <Error<T>>::NotEnoughBalance);

			// Performs this operation first as it may fail
			let new_cnt = Self::name_cnt().checked_add(1)
				.ok_or(<Error<T>>::NameCntOverflow)?;

			// remove entry from old user owned entries
			if old_owner != *owner {
				<NameEntriesOwned<T>>::try_mutate(&old_owner, |owned| {
					if let Some(ind) = owned.iter().position(|&id| id == name_id) {
						owned.swap_remove(ind);
						return Ok(());
					}
					Err(())
				}).map_err(|_| <Error<T>>::NameNotExist)?;	
			}
			// add entry to new user owned entries, only once
			else if !already_registered {
				<NameEntriesOwned<T>>::try_mutate(&owner, |name_entry_vec| {
					name_entry_vec.try_push(name_id)
				}).map_err(|_| <Error<T>>::ExceedMaxNamesOwned)?;
			}

			// save updated name_entry
			<NameEntries<T>>::insert(name_id, name_entry);

			<NameCnt<T>>::put(new_cnt);

			// pay for name registration
			T::Currency::transfer(owner, &Self::account_id(), fee, ExistenceRequirement::KeepAlive)?;

			Ok(eff_expires_at)
		}

		pub fn is_name_owner(name_entry_id: &T::Hash, acct: &T::AccountId) -> Result<bool, Error<T>> {
			match Self::name_entries(name_entry_id) {
				Some(name_entry) => Ok(name_entry.owner == *acct),
				None => Err(<Error<T>>::NameNotExist)
			}
		}

		fn calculate_fee(num_blocks: u32) -> Result<BalanceOf<T>, Error<T>> {
			let maybe_balance = num_blocks.checked_mul(T::BlockReservationCost::get())
				.ok_or(
					<Error<T>>::FeeOverflow
				)?;

			let balance: BalanceOf<T> = maybe_balance.into();

			Ok(balance)
		}
	}
}
