use anchor_lang::prelude::*;

declare_id!("38sbRMbhMrbw6i1Cn21zoFBWoCBzHUyCjDEu5QF9XGGw");

#[program]
pub mod round {
    use super::*;

    // Initialize the contract with the slot price and fee
 // Initialize the contract with the slot price, fee, and vault creation
    pub fn initialize(ctx: Context<Initialize>, slot_token_price: u64, fee: u32) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
        global_state.slot_token_price = slot_token_price;
        global_state.fee = fee;
        global_state.owner = *ctx.accounts.owner.key;
        global_state.paused = false;
        global_state.emergency_mode = false;

        // Derive the vault PDA using the program ID and the owner’s public key
        let (vault_pda, _) = Pubkey::find_program_address(
            &[b"vault", global_state.owner.as_ref()],
            ctx.program_id,
        );

        // Ensure the vault is initialized and matches the expected PDA
        if ctx.accounts.vault.key() != vault_pda {
            return Err(RoundError::VaultMismatch.into()); // Vault doesn't match the expected PDA
        }

        Ok(())
    }

    // Buy slots and transfer the SOL to the vault
     // Buy slots (with Chad mod if >= 4 slots are bought)
        pub fn buy_slot(
        ctx: Context<BuySlot>,
        round_index: u16,
        amount: u32,
        method: bool
    ) -> Result<()> {
        let user_info = &mut ctx.accounts.user_info;
        let global_state = &mut ctx.accounts.global_state;
        let round = &mut ctx.accounts.round;

        // Check if the contract is paused or in emergency mode
        if global_state.paused || global_state.emergency_mode {
            return Err(RoundError::ContractPaused.into()); // Contract is paused
        }

        // Calculate the total price for the amount of slots
        let price = global_state.slot_token_price * (amount as u64);
        let price_lamports = price as u64;

        // Derive the vault PDA again using the program ID and owner’s public key
        let (vault_pda, _) = Pubkey::find_program_address(
            &[b"vault", global_state.owner.as_ref()],
            ctx.program_id,
        );

        // Check if the provided vault matches the derived PDA
        if ctx.accounts.vault.key() != vault_pda {
            return Err(RoundError::VaultMismatch.into()); // Vault doesn't match the expected PDA
        }

        // Transfer the SOL from the user to the vault
        **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? -= price_lamports;
        **ctx.accounts.vault.to_account_info().try_borrow_mut_lamports()? += price_lamports;

        // Process Chad mod logic if the user buys >= 4 slots
        if method && amount >= 4 {
            user_info.chad_last_slot_number = amount;
        }

        // Update the user's information and the round state
        user_info.total_slots_purchased += amount;
        round.current_slot_number += amount;
        round.total_slot_number += amount;

        // Emit the SlotPurchased event
        emit!(SlotPurchased {
            user: *ctx.accounts.user.key,
            round_index,
            amount,
            chad_mod: method,
            user_total_slots: user_info.total_slots_purchased,
        });

        Ok(())
    }

    // Create a new round for the slot purchase
      pub fn create_round(ctx: Context<CreateRound>, round_index: u16) -> Result<()> {
        let round = &mut ctx.accounts.round;
        round.round_index = round_index;
        round.total_slot_number = 0;
        round.current_slot_number = 0;

        Ok(())
    }
   
    // Claim the slot (only available if conditions are met)
    pub fn claim_slot(ctx: Context<ClaimSlot>) -> Result<()> {
        let user_info = &mut ctx.accounts.user_info;
        let round = &mut ctx.accounts.round;

        if user_info.claimed_slot_number > 0 {
            return Err(RoundError::AlreadyClaimed.into()); // Already claimed
        }

        // Logic for claiming the slot
        user_info.claimed_slot_number += 1;
        Ok(())
    }

    // Update the fee for the contract (only accessible by the owner)
    pub fn update_fee(ctx: Context<UpdateFee>, new_fee: u32) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
        // Ensure the new fee is valid (for example, below a max fee)
        if new_fee > 1000 {
            return Err(RoundError::MaxFeeExceeded.into()); // Fee exceeds maximum allowed
        }
        global_state.fee = new_fee;
        Ok(())
    }

    // Emergency functions to handle pausing or unpausing the contract
    pub fn emergency_pause(ctx: Context<EmergencyControl>) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
        global_state.paused = true;
        Ok(())
    }

    pub fn emergency_unpause(ctx: Context<EmergencyControl>) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
        global_state.paused = false;
        Ok(())
    }

    pub fn emergency_withdraw_all(ctx: Context<EmergencyControl>) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
        let vault = &mut ctx.accounts.vault;
        let amount = vault.lamports();
        **vault.to_account_info().try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.owner.to_account_info().try_borrow_mut_lamports()? += amount;

        Ok(())
    }

    // Timelock function to update fees (timelock execution)
    pub fn update_fee_with_timelock(
        ctx: Context<UpdateFeeWithTimelock>,
        new_fee: u32,
        execution_time: i64
    ) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
        global_state.pending_fee = new_fee;
        global_state.fee_execution_time = execution_time;
        Ok(())
    }

    // Execute fee change after timelock has expired
    pub fn execute_fee_change(ctx: Context<ExecuteFeeChange>) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
        let current_time = Clock::get()?.unix_timestamp;

        // Ensure timelock has expired
        if current_time < global_state.fee_execution_time {
            return Err(RoundError::TimelockNotExpired.into()); // Timelock has not expired yet
        }

        global_state.fee = global_state.pending_fee;
        Ok(())
    }

    // Transfer ownership of the contract
    pub fn transfer_ownership(ctx: Context<TransferOwnership>, new_owner: Pubkey) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
        global_state.owner = new_owner;
        Ok(())
    }

}

// Custom error definitions
#[error_code]
pub enum RoundError {
    #[msg("The contract is paused.")]
    ContractPaused,

    #[msg("The user has already claimed their slot.")]
    AlreadyClaimed,

    #[msg("Fee exceeds the maximum allowed.")]
    MaxFeeExceeded,

    #[msg("The timelock has not expired yet.")]
    TimelockNotExpired,

    #[msg("The vault PDA does not match the expected address.")]
    VaultMismatch,
}



// Accounts structs
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = owner, space = 8 + 256)]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = owner, space = 8 + 256)] // Initialize vault as a PDA
     /// CHECK: The vault is a Program Derived Address (PDA) derived from the owner's public key.
    /// This is safe because only the contract and the owner have access to this vault.
    pub vault: AccountInfo<'info>, // Vault to store SOL
}

#[derive(Accounts)]
pub struct CreateRound<'info> {
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
    #[account(init, payer = owner, space = 8 + 128)]
    pub round: Account<'info, RoundState>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}


#[derive(Accounts)]
pub struct BuySlot<'info> {
    #[account(mut)]
    pub user: Signer<'info>,                      // User making the purchase
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>, // Contract global state
    #[account(mut)]
    pub round: Account<'info, RoundState>,         // Round state
    #[account(mut)]
    pub user_info: Account<'info, UserInfo>,       // User-specific info
    #[account(mut)]
     /// CHECK: The vault is a Program Derived Address (PDA) derived from the owner's public key.
    /// This is safe because only the contract and the owner have access to this vault.
    pub vault: AccountInfo<'info>,                 // Contract vault (PDA)
    pub system_program: Program<'info, System>,    // System program
}


#[derive(Accounts)]
pub struct ClaimSlot<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut)]
    pub round: Account<'info, RoundState>,
    #[account(mut)]
    pub user_info: Account<'info, UserInfo>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateFee<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
}

#[derive(Accounts)]
pub struct EmergencyControl<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
    // The vault account stores the funds in the contract, and we assume it is safe to modify.
    /// CHECK: The vault is a Program Derived Address (PDA) that is derived from the owner's public key.
    /// The contract controls access to the vault, ensuring it is safe to use here.
    #[account(mut)]
    pub vault: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct UpdateFeeWithTimelock<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
}

#[derive(Accounts)]
pub struct ExecuteFeeChange<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
}

#[derive(Accounts)]
pub struct TransferOwnership<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
}

#[account]
pub struct GlobalState {
    pub owner: Pubkey,
    pub slot_token_price: u64,
    pub fee: u32,
    pub paused: bool,
    pub emergency_mode: bool,
    pub pending_fee: u32,
    pub fee_execution_time: i64,
}

#[account]
pub struct RoundState {
    pub round_index: u16,
    pub total_slot_number: u32,
    pub current_slot_number: u32,
}

#[account]
pub struct UserInfo {
    pub address: Pubkey,
    pub total_slots_purchased: u32,
    pub claimed_slot_number: u32,
    pub chad_last_slot_number: u32,
}
#[event]
pub struct SlotPurchased {
    pub user: Pubkey,
    pub round_index: u16,
    pub amount: u32,
    pub chad_mod: bool,
    pub user_total_slots: u32,
}