use anchor_lang::prelude::*;
use anchor_lang::solana_program::{clock};
use anchor_spl::token::{self, CloseAccount, Mint, SetAuthority, TokenAccount, Transfer};
use solana_program::{program::invoke_signed, system_instruction};
use std::mem::size_of;

declare_id!("AyR2eU8vpdKBe8bNscxugRm2DpB5frupW3QDFRTfwrRh");

#[program]
pub mod spin {
    use super::*;

    pub const ESCROW_PDA_SEED: &str = "sw_game_vault_auth";
    pub const USER_STATE_SEED: &[u8] = b"USER_STATE_SEED";
    pub const ADMIN_SETTING_SEED: &[u8] = b"ADMIN_SETTING_SEED";
    pub const ADMIN_LIST_SEED: &[u8] = b"ADMIN_LIST_SEED";
    pub const VAULT_SEED: &[u8] = b"SOL_VAULT";

    pub const SPIN_ITEM_COUNT: usize = 15;
    pub const REWARD_TOKEN_COUNT_PER_ITEM: usize = 10;
    pub const ADMIN_MAX_COUNT: usize = 15;
    pub const MAX_REWARD_TOKEN_COUNT: usize = 150; // REWARD_TOKEN_COUNT_PER_ITEM * SPIN_ITEM_COUNT;


    pub fn initialize(
        ctx: Context<Initialize>,
        _bump : u8,
    ) -> Result<()> {
        msg!("initialize");

        let pool = &mut ctx.accounts.pool;
        pool.superadmin = ctx.accounts.super_admin.key();
        pool.bump = _bump;

        let mut _state = ctx.accounts.state.load_init()?;

        Ok(())
    }

    pub fn add_item(
        ctx: Context<SpinWheel>,
        item_mint_list: [Pubkey; 10],
        count: u8,
        token_type: u8,
        ratio: u32,
        amount: u64,
    ) -> Result<()> {
        msg!("add_item");

        let mut state = ctx.accounts.state.load_mut()?;
        state.add_spinitem(ItemRewardMints{item_mint_list, count}, token_type, ratio, amount)?;

        Ok(())
    }

    pub fn set_item(
        ctx: Context<SpinWheel>,
        index: u8,
        item_mint_list: [Pubkey; 10],
        count: u8,
        token_type: u8,
        ratio: u32,
        amount: u64,
    ) -> Result<()> {
        msg!("set_item");

        let mut state = ctx.accounts.state.load_mut()?;
        state.set_spinitem(index, ItemRewardMints{item_mint_list, count}, token_type, ratio, amount)?;

        Ok(())
    }

    pub fn spin_wheel(ctx: Context<PlayGame>, rand: u32, _round_id: u64) -> Result<u8> {
        let accts = ctx.accounts;
        if accts.user_state.is_initialized == 0 {
            accts.user_state.is_initialized = 1;
            accts.user_state.user = accts.user.key();
            accts.user_state.round_num = 1;
        } else {
            require!(
                accts.user_state.user.eq(&accts.user.key()),
                SpinError::IncorrectUserState
            );
            accts.user_state.round_num = accts.user_state.round_num + 1;
        }

        let mut state = accts.state.load_mut()?;
        state.get_spinresult(rand);

        let amount = state.amount_list[state.last_spinindex as usize];
        let reward_mints = state.reward_mint_list[state.last_spinindex as usize];
        for i in 0..reward_mints.count {
            accts.user_pendingstate.add_item(reward_mints.item_mint_list[i as usize], amount)?;
        }

        accts.user_pendingstate.user = accts.user.key();
        accts.user_pendingstate.is_claimed = 0;
        accts.user_pendingstate.round_num = accts.user_state.round_num;

        return Ok(state.last_spinindex);
    }

    pub fn claim(
        ctx : Context<Claim>,
        amount: u64,
        ) -> Result<()> {

        let (_vault_authority, vault_authority_bump) =
        Pubkey::find_program_address(&[ESCROW_PDA_SEED.as_ref()], ctx.program_id);
        let authority_seeds = &[&ESCROW_PDA_SEED.as_bytes()[..], &[vault_authority_bump]];

        token::transfer(
            ctx.accounts.into_transfer_to_pda_context()
                .with_signer(&[&authority_seeds[..]]),
        amount,
        )?;

        Ok(())
    }

    pub fn withdraw_paid_tokens(
        ctx : Context<Withdraw>,
        amount: u64,
        ) -> Result<()> {

        let (_vault_authority, vault_authority_bump) =
        Pubkey::find_program_address(&[ESCROW_PDA_SEED.as_ref()], ctx.program_id);
        let authority_seeds = &[&ESCROW_PDA_SEED.as_bytes()[..], &[vault_authority_bump]];

        token::transfer(
            ctx.accounts.into_transfer_from_pda_context()
                .with_signer(&[&authority_seeds[..]]),
        amount,
        )?;

        Ok(())
    }

    pub fn withdraw_sol( ctx : Context<WithdrawSol>, amount: u64) -> Result<()> {
        let accts = ctx.accounts;

        // send fee to treasury
        let bump = ctx.bumps.get("vault").unwrap();
        invoke_signed(
            &system_instruction::transfer(&accts.vault.key(), &accts.dest_account.key(), amount),
            &[
                accts.vault.to_account_info().clone(),
                accts.dest_account.clone(),
                accts.system_program.to_account_info().clone(),
            ],
            &[&[VAULT_SEED, &[*bump]]],
        )?;

        Ok(())
    }

    pub fn close_user_pending_acc(ctx : Context<CloseUserPendingAcc>) -> Result<()> {
        ctx.accounts.user_pendingstate.is_claimed = 1;

        Ok(())
    }

    pub fn set_settinginfo(ctx : Context<SetSettingInfo>, payment_amount: u64, payment_solamount: u64) -> Result<()> {
        ctx.accounts.setting_info.payment_token = ctx.accounts.payment_token.key();
        ctx.accounts.setting_info.payment_amount = payment_amount;
        ctx.accounts.setting_info.payment_solamount = payment_solamount;

        Ok(())
    }

    pub fn add_admin(ctx : Context<ManageAdmin>) -> Result<()> {
        ctx.accounts.admin_info.add_admin(ctx.accounts.admin.key())?;
        Ok(())
    }

    pub fn delete_admin(ctx : Context<ManageAdmin>) -> Result<()> {
        ctx.accounts.admin_info.delete_admin(ctx.accounts.admin.key())?;
        Ok(())
    }

}


#[account]
#[derive(Default)]
pub struct Pool {
    pub superadmin : Pubkey,
    pub bump : u8,
}

#[derive(Accounts)]
#[instruction(_bump : u8)]
pub struct Initialize<'info> {
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut, signer)]
    pub initializer: AccountInfo<'info>,

    #[account(init, seeds=[ESCROW_PDA_SEED.as_ref()], bump, payer=initializer, space=size_of::<Pool>() + 8)]
    pool : Account<'info, Pool>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    pub super_admin: AccountInfo<'info>,

    #[account(zero)]
    state : AccountLoader<'info, SpinItemList>,

    #[account(init, seeds=[ADMIN_SETTING_SEED], bump, payer=initializer, space=size_of::<SettingInfo>() + 8)]
    setting_info : Account<'info, SettingInfo>,

    #[account(init, seeds=[ADMIN_LIST_SEED], bump, payer=initializer, space=size_of::<AdminInfo>() + 8)]
    admin_info : Account<'info, AdminInfo>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    pub system_program: AccountInfo<'info>,
}

// space : 32 * 10 + 1
#[zero_copy]
#[derive(Default, AnchorSerialize, AnchorDeserialize)]
pub struct ItemRewardMints {
    item_mint_list: [Pubkey; REWARD_TOKEN_COUNT_PER_ITEM],
    count: u8,
}

// space : 5020 // old : 4975
#[account(zero_copy)]
#[repr(packed)]
pub struct SpinItemList {
    reward_mint_list: [ItemRewardMints; SPIN_ITEM_COUNT],   // 321 * 15
    token_type_list: [u8; SPIN_ITEM_COUNT],   // 15
    ratio_list: [u32; SPIN_ITEM_COUNT],  // 4 * 15
    amount_list: [u64; SPIN_ITEM_COUNT],    // 8 * 15
    last_spinindex: u8, // 1
    count: u8, // 1
}

impl ItemRewardMints {
    pub fn add_reward_item(&mut self, reward_mint: Pubkey) {
        self.item_mint_list[self.count as usize] = reward_mint;
        self.count += 1;
    }
}

impl Default for SpinItemList {
    #[inline]
    fn default() -> SpinItemList {
        SpinItemList {
            reward_mint_list: [
                ItemRewardMints {
                    ..Default::default()
                }; SPIN_ITEM_COUNT
            ],
            token_type_list: [0; SPIN_ITEM_COUNT],
            ratio_list: [0; SPIN_ITEM_COUNT],
            amount_list: [0; SPIN_ITEM_COUNT],
            last_spinindex: 0,
            count: 0,
        }
    }
}

impl SpinItemList {
    pub fn add_spinitem(&mut self, item_mint_list: ItemRewardMints, token_type: u8, ratio: u32, amount: u64,) -> Result<()> {
        require!(self.count <= SPIN_ITEM_COUNT as u8, SpinError::CountOverflowAddItem);

        self.reward_mint_list[self.count as usize] = item_mint_list;
        self.token_type_list[self.count as usize] = token_type;
        self.ratio_list[self.count as usize] = ratio;
        self.amount_list[self.count as usize] = amount;
        self.count += 1;

        Ok(())
    }

    pub fn set_spinitem(&mut self, index: u8, item_mint_list: ItemRewardMints, token_type: u8, ratio: u32, amount: u64,) -> Result<()> {
        require!(index < SPIN_ITEM_COUNT as u8, SpinError::IndexOverflowSetItem);

        self.reward_mint_list[index as usize] = item_mint_list;
        self.token_type_list[index as usize] = token_type;
        self.ratio_list[index as usize] = ratio;
        self.amount_list[index as usize] = amount;
        if self.count <= index {
            self.count = index + 1;
        }

        Ok(())
    }

    pub fn clear_spinitem(&mut self) {
        self.count = 0;
    }

    pub fn get_spinresult(&mut self, rand: u32) {
        let ctime = clock::Clock::get().unwrap();
        let c = ctime.unix_timestamp * rand as i64;
        let r = (c % 100) as u32;
        let r_pow = r * ((10 as u32).pow(3));
        let mut start = 0;
        for (pos, item) in self.ratio_list.iter().enumerate() {
            let end = start + item;
            if r_pow >= start && r_pow < end {
                self.last_spinindex = pos as u8;
                return;
            }
            start = end;
        }
    }
}

#[derive(Accounts)]
pub struct SpinWheel<'info> {
    #[account(mut)]
    state : AccountLoader<'info, SpinItemList>,
}

#[derive(Accounts)]
#[instruction(rand: u32, round_id : u64)]
pub struct PlayGame<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    state : AccountLoader<'info, SpinItemList>,

    #[account(
        init_if_needed,
        seeds = [USER_STATE_SEED, user.key().as_ref()],
        bump,
        payer = user,
        space = 8 + size_of::<UserState>()
    )]
    pub user_state: Account<'info, UserState>,

    #[account(
        init,
        seeds = [&round_id.to_le_bytes(), user.key().as_ref()],
        bump,
        payer = user,
        space = 8 + size_of::<UserPendingClaimState>()
    )]
    pub user_pendingstate: Account<'info, UserPendingClaimState>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    pub system_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Claim<'info> {
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut, signer)]
    owner : AccountInfo<'info>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut)]
    pool : Account<'info, Pool>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut,owner=spl_token::id())]
    source_reward_account : AccountInfo<'info>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut,owner=spl_token::id())]
    dest_reward_account : AccountInfo<'info>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,
}

impl<'info> Claim<'info> {
    fn into_transfer_to_pda_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self
                .source_reward_account
                .to_account_info()
                .clone(),
            to: self.dest_reward_account.to_account_info().clone(),
            authority: self.pool.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }
}

#[derive(Accounts)]
pub struct CloseUserPendingAcc<'info> {
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut, signer)]
    owner : AccountInfo<'info>,

    #[account(
        mut,
        close = owner,
    )]
    pub user_pendingstate: Account<'info, UserPendingClaimState>,
}


#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut, constraint = pool.superadmin == *authority.key)]
    pool : Account<'info, Pool>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut,owner=spl_token::id())]
    source_account : AccountInfo<'info>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut,owner=spl_token::id())]
    dest_account : AccountInfo<'info>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,
}

impl<'info> Withdraw<'info> {
    fn into_transfer_from_pda_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self
                .source_account
                .to_account_info()
                .clone(),
            to: self.dest_account.to_account_info().clone(),
            authority: self.pool.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }
}

#[derive(Accounts)]
pub struct WithdrawSol<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [VAULT_SEED],
        bump
    )]
    /// CHECK: this should be checked with address in global_state
    pub vault: AccountInfo<'info>,

    #[account(mut)]
    /// CHECK: this should be checked with address in global_state
    pub dest_account: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetSettingInfo<'info> {
    #[account(mut)]
    setting_info : Account<'info, SettingInfo>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    payment_token : AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct ManageAdmin<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(constraint = pool.superadmin == *authority.key)]
    pool : Account<'info, Pool>,

    #[account(mut)]
    admin_info : Account<'info, AdminInfo>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    admin : AccountInfo<'info>,
}

#[account]
#[derive(Default)]
pub struct UserState {
    // to avoid reinitialization attack
    pub is_initialized: u8,

    // user
    pub user: Pubkey,

    pub round_num: u32,
}

#[account]
#[derive(Default)]
pub struct UserPendingClaimState {
    // user
    pub user: Pubkey,
    pub is_claimed: u8,
    pub round_num: u32,

    pub pending_mint_list: [Pubkey; REWARD_TOKEN_COUNT_PER_ITEM],
    pub pending_amount_list: [u64; REWARD_TOKEN_COUNT_PER_ITEM],
    pub count: u8,
}

impl UserPendingClaimState {
    pub fn add_item(&mut self, pending_mint: Pubkey, amount: u64) -> Result<()> {
        require!(self.count <= REWARD_TOKEN_COUNT_PER_ITEM as u8, SpinError::CountOverflowAddItem);

        self.pending_mint_list[self.count as usize] = pending_mint;
        self.pending_amount_list[self.count as usize] = amount;
        self.count += 1;

        Ok(())
    }
}

#[account]
#[derive(Default)]
pub struct SettingInfo {
    pub payment_token: Pubkey,
    pub payment_amount: u64,
    pub payment_solamount: u64,
}

#[account]
#[derive(Default)]
pub struct AdminInfo {
    pub admin_list: [Pubkey; ADMIN_MAX_COUNT],
    pub count: u8,
}

impl AdminInfo {
    pub fn add_admin(&mut self, admin: Pubkey) -> Result<()> {
        require!(self.count <= ADMIN_MAX_COUNT as u8, SpinError::CountOverflowAddItem);

        self.admin_list[self.count as usize] = admin;
        self.count += 1;

        Ok(())
    }

    pub fn delete_admin(&mut self, admin: Pubkey) -> Result<()> {
        for i in 0..self.count {
            if self.admin_list[i as usize].eq(&admin) {
                self.admin_list[i as usize] = self.admin_list[self.count as usize - 1];
                self.count -= 1;
                break;
            }
        }

        Ok(())
    }
}

#[error_code]
pub enum SpinError {
    #[msg("Count Overflow To Add Item")]
    CountOverflowAddItem,

    #[msg("Index Overflow To Set Item")]
    IndexOverflowSetItem,

    #[msg("Incorrect User State")]
    IncorrectUserState,

    #[msg("Incorrect Claim Amount")]
    ClaimAmountError,
}