use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Vault {
    /// The vault's name.
    pub name: [u8; 32],

    /// A description or summary for this vault.
    pub description: [u8; 32],
    pub description2: [u8; 32],

    /// The vault’s main asset configuration (inline nested struct).
    pub asset: VaultAsset,

    /// The vault’s LP (share) configuration (inline nested struct).
    pub lp: VaultLp,

    /// The manager of this vault (has certain permissions).
    pub manager: Pubkey,

    /// The admin of this vault (broader or fallback permissions).
    pub admin: Pubkey,

    /// The vault fee, cap, and locked profit degradation duration configuration (inline nested struct).
    pub vault_configuration: VaultConfiguration,

    /// The vault fee and cap configuration (inline nested struct).
    pub fee_configuration: FeeConfiguration,

    /// The fee update state of the vault.
    pub fee_update: FeeUpdate,

    /// The fee state of the vault.
    pub fee_state: FeeState,

    pub high_water_mark: HighWaterMark,

    /// The last time (Unix timestamp) this vault data was updated.
    pub last_updated_ts: u64,

    /// The version of the vault.
    pub version: u8,

    /// padding to align future 8-byte fields on 8-byte boundaries.
    pub _padding0: [u8; 7],

    /// The locked profit state of the vault.
    pub locked_profit_state: LockedProfitState,

    /// Reserved bytes for future use.
    pub reserved: [u8; 32],
    pub reserved2: [u8; 32],
    pub reserved3: [u8; 32],
    pub reserved4: [u8; 32],
    pub reserved5: [u8; 32],
    pub reserved6: [u8; 32],
    pub reserved7: [u8; 32],
    pub reserved8: [u8; 16],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LockedProfitState {
    pub last_updated_locked_profit: u64,
    pub last_report: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VaultAsset {
    /// The mint for the vault’s main asset.
    pub mint: Pubkey, // offset 0..32

    /// The “idle” token account holding un-invested assets.
    pub idle_ata: Pubkey, // offset 32..64

    /// The total amount of this asset currently in the vault.
    pub total_value: u64, // offset 64..72

    /// The bump for the vault asset mint.
    pub idle_ata_auth_bump: u8, // offset 72..73

    /// Reserved bytes for future use.
    pub reserved: [u8; 32], // offset 73..168
    pub reserved2: [u8; 32],
    pub reserved3: [u8; 31],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VaultLp {
    /// The LP mint (e.g., representing shares in this vault).
    pub mint: Pubkey, // offset 0..32

    /// The bump for the vault LP mint.
    pub mint_bump: u8, // offset 64..65

    /// The bump for the vault LP mint authority.
    pub mint_auth_bump: u8, // offset 65..66

    /// Reserved bytes for future use.
    pub reserved: [u8; 32], // offset 67..160
    pub reserved2: [u8; 30], // offset 160..222
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VaultConfiguration {
    /// The maximum total amount allowed in the vault.
    pub max_cap: u64, // offset 0..8

    /// active from timestamp
    pub start_at_ts: u64, // offset 8..16

    /// The locked profit degradation duration.
    pub locked_profit_degradation_duration: u64, // offset 16..24

    /// The waiting period for a withdrawal. prec: seconds
    pub withdrawal_waiting_period: u64, // offset 24..32

    /// Reserved bytes for future use.
    pub reserved: [u8; 32], // offset 32..80
    pub reserved2: [u8; 16], // offset 80..84
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FeeConfiguration {
    /// Manager performance fee in basis points (BPS).
    pub manager_performance_fee: u16, // offset 0..2

    /// Admin performance fee in basis points (BPS).
    pub admin_performance_fee: u16, // offset 2..4

    /// Manager management fee in basis points (BPS).
    pub manager_management_fee: u16, // offset 4..6

    /// Admin management fee in basis points (BPS).
    pub admin_management_fee: u16, // offset 6..8

    /// The redemption fee in basis points (BPS).
    pub redemption_fee: u16, // offset 8..10

    /// The issuance fee in basis points (BPS).
    pub issuance_fee: u16, // offset 10..12

    /// Reserved bytes for future use.
    pub reserved: [u8; 32], // offset 12..48

    /// Reserved bytes for future use.
    pub reserved2: [u8; 4], // offset 48..52
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct HighWaterMark {
    /// The highest recorded total asset value per share
    pub highest_asset_per_lp_decimal_bits: u128,
    /// The timestamp when the high water mark was last updated
    pub last_updated_ts: u64,
    /// Reserved for future use
    pub reserved: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FeeUpdate {
    /// The timestamp when the performance fees were last updated.
    pub last_performance_fee_update_ts: u64,

    /// The timestamp when the management fees were last updated.
    pub last_management_fee_update_ts: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FeeState {
    /// The accumulated manager fees in the vault.
    pub accumulated_lp_manager_fees: u64,

    /// The accumulated admin fees in the vault.
    pub accumulated_lp_admin_fees: u64,

    /// The accumulated protocol fees in the vault.
    pub accumulated_lp_protocol_fees: u64,

    /// Reserved bytes for future use.
    pub reserved: [u8; 24],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VaultInitializationInput {
    /// The maximum total amount allowed in the vault.
    pub max_cap: u64,
    /// active from timestamp
    pub start_at_ts: u64,
    /// Manager performance fee in basis points (BPS).
    pub manager_performance_fee: u16,
    /// Admin performance fee in basis points (BPS).
    pub admin_performance_fee: u16,
    /// Manager management fee in basis points (BPS).
    pub manager_management_fee: u16,
    /// Admin management fee in basis points (BPS).
    pub admin_management_fee: u16,
    /// The locked profit degradation duration.
    pub locked_profit_degradation_duration: u64,
    /// The redemption fee in basis points (BPS).
    pub redemption_fee: u16,
    /// The issuance fee in basis points (BPS).
    pub issuance_fee: u16,
    /// The waiting period for a withdrawal.
    pub withdrawal_waiting_period: u64,
}

impl<'info> TryFrom<&'info [u8]> for &'info Vault {
    type Error = ProgramError;
    fn try_from(data: &'info [u8]) -> Result<Self, Self::Error> {
        if data.len() != core::mem::size_of::<Vault>() + 8 {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &*(data[8..].as_ptr() as *const Vault) })
    }
}
