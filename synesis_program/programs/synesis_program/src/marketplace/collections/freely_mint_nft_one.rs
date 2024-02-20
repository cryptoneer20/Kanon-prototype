use crate::globals::*;
use anchor_lang::{prelude::*, solana_program::{system_instruction, program::invoke}};
use anchor_spl::token::{Token, TokenAccount, Transfer, self};
use crate::globals::constants::constants::*;
////////////////////////////////
/// Escrow 2SOL and transfer NFT to user from NFT Authority
/// param:
/// - season_number
/// - amount
/// action:
/// - step should be FREELYMINT
/// - check sold amount limitation
/// - check time limitation for freely mint
/// - check user's total quantity (max 1 - global state)
///
/// - create or update NFT_Reserve_PDA
/// - check amount 2 SOL (global state)
/// - escrow funds
/// - transfer nft
/// - increase sold_art_amount
///
/// /// emit event
/// //emit event for start of season
// emit!(event::SeasonStepEvent {
//     season_number: self.global_state.season_number,
//     current_step: self.collection_state.current_step,
// });
////////////////////////////////

#[derive(Accounts)]
#[instruction(args: FreelyMintNftOneArgs)]
pub struct FreelyMintNftOne<'info> {
    #[account(
        address = KANON_STATE_ACCOUNT_PUBKEY.parse::<Pubkey>().unwrap() @ ErrorCode::InvalidGlobalAccount,
        constraint = global_state.season_number == args.season_number @ ErrorCode::WrongSeasonNumber,
    )]
    pub global_state: Box<Account<'info, GlobalAccount>>,

    #[account(
        seeds = [
            global_state.key().as_ref(),
            ADMIN_TREASURY_ACCOUNT_PREFIX.as_ref(),
            KANON_GLOBAL_SEED.as_ref(),
        ],
        bump = global_state.admin_treasury_account_bump,
    )]
    pub admin_treasury_account: AccountInfo<'info>,

    #[account(
        mut,
        constraint = collection_state.get_current_step(clock.unix_timestamp) == SeasonStep::FREELYMINT && 
        collection_state.sold_art_amount < (collection_state.art_amount - collection_state.promos_reserved_nfts_amount) &&
        args.cost == NFT_MINT_COST @ ErrorCode::ConditionMismatch,
        seeds = [ 
            global_state.key().as_ref(), 
            COLLECTION_STATE_PREFIX.as_ref(), 
            KANON_GLOBAL_SEED.as_ref(), 
            KANON_GLOBAL_SEASON_SEED.as_ref(), 
            &[args.season_number].as_ref() 
        ],
        bump = collection_state.bump,
    )]
    pub collection_state: Box<Account<'info, CollectionAccount>>,

    #[account(
        seeds = [ 
            collection_state.key().as_ref(), 
            COLLECTION_AUTHORITY_PREFIX.as_ref(), 
            KANON_GLOBAL_SEED.as_ref(), 
            KANON_GLOBAL_SEASON_SEED.as_ref(), 
            &[args.season_number].as_ref() 
        ],
        bump = collection_state.authority_bump,
    )]
    pub collection_authority: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [
            collection_state.key().as_ref(), 
            USER_MINT_RESERVE_STATE_PREFIX.as_ref(), 
            KANON_GLOBAL_SEED.as_ref(), 
            KANON_GLOBAL_SEASON_SEED.as_ref(), 
            &[args.season_number].as_ref(),
            user.key().as_ref(), 
        ],
        bump = user_mint_reserve_state.bump,
    )]
    pub user_mint_reserve_state: Box<Account<'info, UserMintReserveAccount>>,

    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub buyer_nft_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub seller_nft_token_account: Box<Account<'info, TokenAccount>>,
    
    pub(crate) clock: Sysvar<'info, Clock>,
    
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
    pub token_program: Program<'info, Token>,
}

impl<'info> FreelyMintNftOne<'info> {
    pub(crate) fn transfer_nft(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.seller_nft_token_account.to_account_info(),
            to: self.buyer_nft_token_account.to_account_info(),
            authority: self.collection_authority.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

impl<'info> Processor<FreelyMintNftOneArgs> for FreelyMintNftOne<'info> {
    fn process(&mut self, args: FreelyMintNftOneArgs) -> ProgramResult {
        // check user's total quantity
        if self.user_mint_reserve_state.freely_minted_amount >= self.collection_state.max_freely_mint_quantity
        {
            return Err(ErrorCode::OutOfUserMaxQuantity.into());
        }
        
        // escrow funds
        if !self.user.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if **self.user.lamports.borrow() < args.cost {
            return Err(ErrorCode::NotEnoughBalanceInUserWallet.into());
        }

        invoke(
            &system_instruction::transfer(
                self.user.key,
                self.admin_treasury_account.key,
                args.cost,
            ),
            &[
                self.user.to_account_info(),
                self.admin_treasury_account.clone(),
                self.system_program.to_account_info(),
            ],
        )?;

        // transfer nft

        let collection_state_key = self.collection_state.key();
        let season_number = &[args.season_number];
        
        let authority_seeds = [
            collection_state_key.as_ref(), 
            COLLECTION_AUTHORITY_PREFIX.as_ref(), 
            KANON_GLOBAL_SEED.as_ref(), 
            KANON_GLOBAL_SEASON_SEED.as_ref(), 
            season_number.as_ref(),
            &[self.collection_state.authority_bump],
        ];

        token::transfer(
            self
            .transfer_nft()
            .with_signer(&[&authority_seeds[..]]),
            1 as u64,
        )?;
        
        
        // create or update NFT_Reserve_PDA
        self.user_mint_reserve_state.freely_minted_amount += 1;

        // increase sold_art_amount in collection state
        self.collection_state.sold_art_amount += 1;

        // close token acct ?????

        //emit event for current sold amount
        emit!(event::SeasonMintEvent {
            season_number: self.global_state.season_number,
            sold_art_amount: self.collection_state.sold_art_amount,
        });

        Ok(())
    }
}



////////////////////////////////
/// Args
////////////////////////////////

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct FreelyMintNftOneArgs {
    pub season_number: u8,
    pub whitelist_proof: Vec<[u8; 32]>,
    pub cost: u64,
    pub user_mint_reserve_bump: u8,
}
