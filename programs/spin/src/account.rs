use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::*;


#[account]
#[derive(Default)]
pub struct Pool {
    pub superadmin: Pubkey,
    pub dev_wallet: Pubkey,
    pub dust_mint: Pubkey,
    pub forge_mint: Pubkey,
    pub sol_price: u64,
    pub dust_price: u64,
    pub forge_price: u64,
    pub dev_fee: u64,
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


// space : 32 * 10 + 1
#[zero_copy]
#[derive(Default, AnchorSerialize, AnchorDeserialize)]
pub struct ItemRewardMints {
    pub item_mint_list: [Pubkey; REWARD_TOKEN_COUNT_PER_ITEM],
    pub count: u8,
}

// space : 5020 // old : 4975
#[account(zero_copy)]
#[repr(packed)]
pub struct SpinItemList {
    pub reward_mint_list: [ItemRewardMints; SPIN_ITEM_COUNT],   // 321 * 15
    pub token_type_list: [u8; SPIN_ITEM_COUNT],   // 15
    pub ratio_list: [u32; SPIN_ITEM_COUNT],  // 4 * 15
    pub amount_list: [u64; SPIN_ITEM_COUNT],    // 8 * 15
    pub last_spinindex: u8, // 1
    pub count: u8, // 1
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
        let ctime = Clock::get().unwrap();
        let c = ctime.unix_timestamp * rand as i64;
        let mut r = (c % 101) as u32;
        if r == 100 {
            r = 99;
        }

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
