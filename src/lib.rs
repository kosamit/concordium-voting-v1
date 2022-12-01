//! This is the first voting contract.
//! The way to create a voting agenda is to initialize the contract with the title, description and proposals as parameters.
//! The function that gives voting rights is excluded in this version.
//! The code related to the ability to grant voting rights is commented out.
//!
//! The current specifications are as follows.
//! - You can vote with any account.
//! - Each account has one vote.
//! - You can change the options until the voting is completed.
//!
//! **WARNING** In this version you can do the following for testing:
//! - Even after the deadline has passed, you can vote if the data is not counted.
//! - Anyone can execute the aggregation method.
//! - Aggregation is possible even before the deadline.

use concordium_std::*;

type ProposalId = u8;
type ProposalNames = Vec<String>;
type Title = String;
type Description = String;
type VoteCount = u32;

#[derive(Debug, Serialize, SchemaType, Default, PartialEq)]
struct VoterState {
    weight: u32,
    voted: bool,
    vote: ProposalId,
}

impl VoterState {
    fn new(weight: u32, voted: bool, vote: ProposalId) -> Self {
        VoterState {
            weight,
            voted,
            vote,
        }
    }
}

#[derive(Debug, Serialize, SchemaType, Default, PartialEq, Clone)]
struct Proposal {
    name: String,
    vote_count: VoteCount,
}

#[derive(Serialize, SchemaType)]
struct InitParams {
    title: Title,
    description: Description,
    proposal_names: ProposalNames,
    expiry: Timestamp,
}

impl Proposal {
    fn new(name: String, vote_count: VoteCount) -> Self {
        Proposal {
            name,
            vote_count: vote_count.into(),
        }
    }
}

#[derive(Serialize, SchemaType)]
struct GetVoterParams {
    voter_address: AccountAddress,
}

#[derive(Serialize, SchemaType)]
struct GetVoteParams {
    proposal_id: ProposalId,
}

/// Contract error type
#[derive(Serialize, Debug, PartialEq, Eq, Reject, SchemaType)]
enum ContractError {
    /// Failed parsing the parameter.
    #[from(ParseError)]
    ParseParams,
    /// Failed logging: Log is full.
    LogFull,
    /// Failed logging: Log is malformed.
    LogMalformed,
    /// The transfer is not from the owner of the vote.
    // FromIsNotTheOwner,
    /// The voter already voted.
    // AlreadyVoted,
    /// The voter already has right to vote.
    // AlreadyHasRightToVote,
    /// The voter doesn't have right to vote.
    // NoRightToVote,
    /// Already finished.
    AlreadyFinished,
    /// exipred for voting.
    Expired,
    /// not exipred for tallying.
    // NotExpired,
    /// Voter is not found.
    VoterIsNotFound,
    /// Voter did not vote.
    NotVoted,
    /// Proposal is not found.
    ProposalIsNotFound,
    /// Sender must be AccountAddress
    ContractSender
}

// [TODO]: ロギング用のイベントの定義をする。
/// Event to be printed in the log.
#[derive(Serialize)]
enum Event {
    GiveRightToVote {
        to: AccountAddress,
        added_weight: u32,
        total_weight: u32,
    },
}

type ContractResult = Result<(), ContractError>;

impl From<LogError> for ContractError {
    fn from(le: LogError) -> Self {
        match le {
            LogError::Full => Self::LogFull,
            LogError::Malformed => Self::LogMalformed,
        }
    }
}

#[derive(Debug, Serialize, SchemaType, Eq, PartialEq, PartialOrd, Clone, Copy)]
enum Status {
    InProcess,
    Finished,
}

#[derive(Serial, DeserialWithState)]
#[concordium(state_parameter = "S")]
struct State<S> {
    voters: StateMap<AccountAddress, VoterState, S>,
    proposals: StateMap<ProposalId, Proposal, S>,
    status: Status,
    winning_proposal_id: StateSet<ProposalId, S>,
    title: Title,
    description: Description,
    expiry: Timestamp,
}

impl<S: HasStateApi> State<S> {
    fn empty(
        title: Title,
        description: Description,
        proposal_names: ProposalNames,
        expiry: Timestamp,
        state_builder: &mut StateBuilder<S>,
    ) -> Self {
        let mut proposals = state_builder.new_map();
        for (i, proposal_name) in proposal_names.iter().enumerate() {
            proposals.insert(i as ProposalId, Proposal::new(proposal_name.to_string(), 0));
        }
        State {
            voters: state_builder.new_map(),
            proposals,
            status: Status::InProcess,
            winning_proposal_id: state_builder.new_set(),
            title,
            description,
            expiry,
        }
    }

    fn vote(&mut self, voter_address: &AccountAddress, proposal_id: &ProposalId) -> ContractResult {
        let mut voter_state = self
            .voters
            .entry(*voter_address)
            .or_insert_with(VoterState::default);

        // 投票済みならその分の投票数を減らす
        if voter_state.voted == true {
            let mut proposal = self
                .proposals
                .get_mut(&voter_state.vote)
                .ok_or(ContractError::ProposalIsNotFound)?;
            proposal.vote_count -= voter_state.weight;
        }

        let mut proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or(ContractError::ProposalIsNotFound)?;

        voter_state.weight = 1;
        voter_state.voted = true;
        voter_state.vote = *proposal_id;
        proposal.vote_count += voter_state.weight;

        Ok(())
    }

    fn cancel_vote(&mut self, voter_address: &AccountAddress) -> ContractResult {
        let mut voter_state = self
            .voters
            .get_mut(&voter_address)
            .ok_or(ContractError::VoterIsNotFound)?;

        ensure_eq!(voter_state.voted, true, ContractError::NotVoted);

        let mut proposal = self
            .proposals
            .get_mut(&voter_state.vote)
            .ok_or(ContractError::ProposalIsNotFound)?;

        proposal.vote_count -= voter_state.weight;

        voter_state.voted = false;
        voter_state.vote = 0;
        Ok(())
    }
}

#[derive(Serialize, SchemaType)]
struct ViewState {
    voters: Vec<(AccountAddress, VoterState)>,
    proposals: Vec<(ProposalId, Proposal)>,
    status: Status,
    winning_proposal_id: Vec<ProposalId>,
    title: Title,
    description: Description,
    expiry: Timestamp,
}

/// Init function that creates a new contract.
#[init(contract = "govote_voting_v1", parameter = "InitParams")]
fn contract_init<S: HasStateApi>(
    ctx: &impl HasInitContext,
    state_builder: &mut StateBuilder<S>,
) -> InitResult<State<S>> {
    let params: InitParams = ctx.parameter_cursor().get()?;
    let state = State::empty(
        params.title,
        params.description,
        params.proposal_names,
        params.expiry,
        state_builder,
    );
    Ok(state)
}

/// Vote to proposal.
#[receive(
    contract = "govote_voting_v1",
    name = "vote",
    parameter = "GetVoteParams",
    mutable
)]
fn contract_vote<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &mut impl HasHost<State<S>, StateApiType = S>,
) -> ContractResult {
    let params: GetVoteParams = ctx.parameter_cursor().get()?;
    let sender = ctx.sender();
    let sender_address = match sender {
        Address::Contract(_) => bail!(ContractError::ContractSender),
        Address::Account(account_address) => account_address,
    };
    let state = host.state_mut();

    // proposalが存在すれば実行できる。
    state
        .proposals
        .get(&params.proposal_id)
        .ok_or(ContractError::ProposalIsNotFound)?;

    // 集計が終わってなければ実行できる。
    ensure!(
        state.status != Status::Finished,
        ContractError::AlreadyFinished
    );

    // expiryを超えていなければ実行できる。
    let slot_time = ctx.metadata().slot_time();
    ensure!(slot_time <= state.expiry, ContractError::Expired);

    state.vote(&sender_address, &params.proposal_id)?;

    Ok(())
}

/// 集計
#[receive(contract = "govote_voting_v1", name = "winningProposal", mutable)]
fn contract_winning_proposal<S: HasStateApi>(
    _ctx: &impl HasReceiveContext,
    host: &mut impl HasHost<State<S>, StateApiType = S>,
) -> ContractResult {
    let mut winning_vote_count = 0;
    let state = host.state_mut();

    // 集計が終わってなければ実行できる。
    ensure!(
        state.status != Status::Finished,
        ContractError::AlreadyFinished
    );

    // expiryを超えていれば実行できる。
    // let slot_time = ctx.metadata().slot_time();
    // ensure!(state.expiry < slot_time, ContractError::NotExpired);

    for (proposal_id, proposal) in state.proposals.iter() {
        if winning_vote_count < proposal.vote_count {
            winning_vote_count = proposal.vote_count;
            state.winning_proposal_id.clear();
            state.winning_proposal_id.insert(*proposal_id);
        } else if winning_vote_count == proposal.vote_count {
            state.winning_proposal_id.insert(*proposal_id);
        };
    }

    state.status = Status::Finished;
    // host.state_mut().winning_proposal_id = winning_proposal_id;

    Ok(())
}

/// 投票のキャンセル
#[receive(contract = "govote_voting_v1", name = "cancelVote", mutable)]
fn cancel_vote<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &mut impl HasHost<State<S>, StateApiType = S>,
) -> ContractResult {
    let sender = ctx.sender();
    let sender_address = match sender {
        Address::Contract(_) => bail!(ContractError::ContractSender),
        Address::Account(account_address) => account_address,
    };
    let state = host.state_mut();

    // 集計が終わってなければ実行できる。
    ensure!(
        state.status != Status::Finished,
        ContractError::AlreadyFinished
    );

    // expiryを超えていなければ実行できる。
    let slot_time = ctx.metadata().slot_time();
    ensure!(slot_time <= state.expiry, ContractError::Expired);

    state.cancel_vote(&sender_address)?;

    Ok(())
}

/// View function.
#[receive(
    contract = "govote_voting_v1",
    name = "view",
    return_value = "ViewState"
)]
fn contract_view<S: HasStateApi>(
    _ctx: &impl HasReceiveContext,
    host: &impl HasHost<State<S>, StateApiType = S>,
) -> ReceiveResult<ViewState> {
    let state = host.state();

    let mut voters = Vec::new();
    for (k, voter) in state.voters.iter() {
        voters.push((*k, VoterState::new(voter.weight, voter.voted, voter.vote)));
    }

    let mut proposals = Vec::new();
    for (k, proposal) in state.proposals.iter() {
        proposals.push((
            *k,
            Proposal::new(proposal.name.to_string(), proposal.vote_count),
        ));
    }
    let winning_proposal_id = state.winning_proposal_id.iter().map(|x| *x).collect();

    Ok(ViewState {
        voters,
        proposals: proposals,
        status: state.status,
        winning_proposal_id: winning_proposal_id,
        title: state.title.to_string(),
        description: state.description.to_string(),
        expiry: state.expiry,
    })
}
