use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};

declare_id!("DkHqJ5CHkBTC3zHuiAZZBvnFgT5DZX9cnxzrk4H1gXmn"); // this program will be deployed on chain with this program id

#[program]
pub mod anchor_vault_q4_25 {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.initialize(&ctx.bumps)
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        ctx.accounts.deposit(amount)
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        ctx.accounts.withdraw(amount)
    }

    pub fn close(ctx: Context<Close>) -> Result<()> {
        ctx.accounts.close()
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    // all the accounts necessary for Initialize instruction
    // Every instruction has its own Context struct that lists all the accounts and, optionally, any data the instruction will need.
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        init,
        payer = user,
        seeds = [b"state", user.key().as_ref()], 
        bump,
        space = VaultState::DISCRIMINATOR.len() + VaultState::INIT_SPACE,
    )]
    pub vault_state: Account<'info, VaultState>,

    #[account(
        mut,
        seeds = [b"vault", vault_state.key().as_ref()],
        bump,
    )]
    pub vault: SystemAccount<'info>, // we dont initialize here because we do it when sending sol.? we are not doing anything with it yet.

    pub system_program: Program<'info, System>,
}

impl<'info> Initialize<'info> {
    pub fn initialize(&mut self, bumps: &InitializeBumps) -> Result<()> {
        // Get the amount of lamports needed to make the vault rent exempt
        let rent_exempt = Rent::get()?.minimum_balance(self.vault.to_account_info().data_len());

        // Transfer the rent-exempt amount from the user to the vault
        let cpi_program = self.system_program.to_account_info();
        let cpi_accounts = Transfer {
            from: self.user.to_account_info(),
            to: self.vault.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        transfer(cpi_ctx, rent_exempt)?;

        self.vault_state.vault_bump = bumps.vault;
        self.vault_state.state_bump = bumps.vault_state;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault", vault_state.key().as_ref()], 
        bump = vault_state.vault_bump,
    )]
    pub vault: SystemAccount<'info>,

    #[account(
        seeds = [b"state", user.key().as_ref()],
        bump = vault_state.state_bump,
    )]
    pub vault_state: Account<'info, VaultState>,

    pub system_program: Program<'info, System>,
}

impl<'info> Deposit<'info> {
    pub fn deposit(&mut self, amount: u64) -> Result<()> {
        let cpi_program = self.system_program.to_account_info();

        let cpi_accounts = Transfer {
            from: self.user.to_account_info(),
            to: self.vault.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        transfer(cpi_ctx, amount)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault", vault_state.key().as_ref()],
        bump = vault_state.vault_bump,
    )]
    pub vault: SystemAccount<'info>,

    #[account(
        seeds = [b"state", user.key().as_ref()], 
        bump = vault_state.state_bump,
    )]
    pub vault_state: Account<'info, VaultState>,

    pub system_program: Program<'info, System>,
}

impl<'info> Withdraw<'info> {
    pub fn withdraw(&mut self, amount: u64) -> Result<()> {
        let cpi_program = self.system_program.to_account_info();

        let cpi_accounts = Transfer {
            from: self.vault.to_account_info(),
            to: self.user.to_account_info(),
        };

        // Rust does NOT auto-convert arrays to slices in nested contexts. so : &[&[u8]] is needed here
        let signer_seeds: &[&[u8]] = &[
            b"vault",
            self.vault_state.to_account_info().key.as_ref(),
            &[self.vault_state.vault_bump],
        ];

        let signer_seeds = &[signer_seeds];

        // new_with_signer supports multiple PDA signers.
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

        transfer(cpi_ctx, amount)?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault", vault_state.key().as_ref()],
        bump = vault_state.vault_bump,
    )]
    pub vault: SystemAccount<'info>,

    #[account(
        mut,
        close = user, // when closing the vault account, send remaining lamports(rent exempt) to user
        seeds = [b"state", user.key().as_ref()],
        bump = vault_state.state_bump,
    )]
    pub vault_state: Account<'info, VaultState>,

    pub system_program: Program<'info, System>,
}

impl<'info> Close<'info> {
    pub fn close(&mut self) -> Result<()> {
        let cpi_program = self.system_program.to_account_info();

        let cpi_account = Transfer {
            from: self.vault.to_account_info(),
            to: self.user.to_account_info(),
        };

        let signer_seeds: &[&[u8]] = &[
            b"vault",
            self.vault_state.to_account_info().key.as_ref(),
            &[self.vault_state.vault_bump],
        ];
        let signer_seeds = &[signer_seeds];
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_account, signer_seeds);
        transfer(cpi_ctx, self.vault.to_account_info().lamports())?;
        Ok(())
    }
}

// #[error_code]
// pub enum VaultError {
//     #[msg("Vault already exists")]
//     VaultAlreadyExists,
//     #[msg("Invalid amount")]
//     InvalidAmount,
// }

#[derive(InitSpace)]
#[account]
pub struct VaultState {
    // saving of the bumps of the pdas so that we prevent using a lot of compute units to find them again
    // find_program_address() use more compute units while reate_program_address() use less.
    pub vault_bump: u8,
    pub state_bump: u8,
}
// impl Space for VaultState {
//     const INIT_SPACE: std::mem::size_of::<VaultState>; // this one also return the size of the VaultState struct
// }



// NOTES:
// macros generate some extra code
// #[program]: Marks the module that contains every instruction entrypoint and business-logic function.
// #[error_code]: Defines custom, human-readable error types that make debugging clearer and faster.
// #[derive(Accounts)]: Lists the accounts an instruction requires and enforces their constraints automatically.

//    #[derive(Accounts)] macro serves three critical responsibilities:
//     Declares all the accounts a specific instruction needs.
//     Enforce constraint checks automatically, blocking many bugs and potential exploits at runtime.
//     Generates helper methods that let you access and mutate accounts safely.

// Account types in this example

//     Signer<'info>: Verifies the account signed the transaction; essential for security and for CPIs that demand a signature.

//     SystemAccount<'info>: Confirms ownership of the account by the System Program.

//     Program<'info, System>: Ensures the account is executable and matches the System Program ID, enabling CPIs such as account creation or lamport transfers.

//     mut: Flags the account as mutable.
//     seeds & bump: Verifies the account is a Program-Derived Address (PDA) generated from the provided seeds plus a bump byte.
