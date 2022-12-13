//! A simple tool for constructing transactions from the command line.

use clap::Parser;
use std::path::PathBuf;

/// Construct a transaction.
#[derive(Debug, Parser)]
#[clap(version, about)]
pub enum Command {
    Create(Create),
    Script(Script),
}

/// Construct a `Create` transaction (e.g. for deploying a contract).
#[derive(Debug, Parser)]
pub struct Create {
    #[clap(flatten)]
    pub gas: Gas,
    #[clap(flatten)]
    pub maturity: Maturity,
    /// Added salt used to derive the contract ID.
    #[clap(long)]
    pub salt: Option<fuel_tx::Salt>,
    /// Path to the contract bytecode.
    #[clap(long)]
    pub bytecode: PathBuf,
    /// Witness index of contract bytecode to create.
    #[clap(long, default_value_t = 0)]
    pub bytecode_witness_index: u8,
    /// Path to a JSON file with a list of storage slots to initialize (key, value).
    #[clap(long)]
    pub storage_slots: PathBuf,
    /// An arbitrary length string of hex-encoded bytes (e.g. "1F2E3D4C5B6A")
    ///
    /// Can be specified multiple times.
    #[clap(long = "witness", multiple = true, max_values = 255)]
    pub witnesses: Vec<String>,
    // TODO: inputs, ouputs
}

/// Construct a `Mint` transaction (e.g. for emulating a block producer).
#[derive(Debug, Parser)]
pub struct Mint {
    /// The location of the `Mint` transaction in the block.
    #[clap(long)]
    pub tx_ptr: fuel_tx::TxPointer,
    // TODO: outputs
}

/// Construct a `Script` transaction (e.g. for running a script).
#[derive(Debug, Parser)]
pub struct Script {
    #[clap(flatten)]
    pub gas: Gas,
    #[clap(flatten)]
    pub maturity: Maturity,
    /// Script to execute.
    #[clap(long)]
    pub bytecode: PathBuf,
    /// Script input data (parameters). Specified file is loaded as raw bytes.
    #[clap(long)]
    pub data: PathBuf,
    /// Merkle root of receipts.
    #[clap(long)]
    pub receipts_root: fuel_tx::Bytes32,
    /// An arbitrary length string of hex-encoded bytes (e.g. "1F2E3D4C5B6A")
    ///
    /// Can be specified multiple times.
    #[clap(long = "witness", multiple = true, max_values = 255)]
    pub witnesses: Vec<String>,
    // TODO: inputs, ouputs
}

/// Flag set for specifying gas price and limit.
#[derive(Debug, Parser)]
pub struct Gas {
    /// Gas price for the transaction.
    ///
    /// Defaults to `fuel_tx::TxParameters::DEFAULT.gas_limit`.
    #[clap(long = "gas-price")]
    pub price: u64,
    /// Gas limit for the transaction.
    #[clap(long = "gas-limit")]
    pub limit: u64,
}

/// Block until which tx cannot be included.
#[derive(Debug, Parser)]
pub struct Maturity {
    /// Block until which tx cannot be included.
    #[clap(long = "maturity")]
    pub field: u32,
}

/// Transaction input.
#[derive(Debug, Parser)]
pub enum Input {
    Coin(InputCoin),
    Contract(InputContract),
    Message(InputMessage),
}

#[derive(Debug, Parser)]
pub struct InputCoin {
    /// Hash of the unspent transaction.
    #[clap(long)]
    pub utxo_id: fuel_tx::UtxoId,
    /// Index of transaction output.
    #[clap(long)]
    pub output_ix: u8,
    /// Owning address or predicate root.
    #[clap(long)]
    pub owner: fuel_tx::Address,
    /// Amount of coins.
    #[clap(long)]
    pub amount: u64,
    /// Asset ID of the coins.
    #[clap(long)]
    pub asset_id: fuel_tx::AssetId,
    /// Points to the TX whose output is being spent. Includes block height, tx index.
    #[clap(long)]
    pub tx_ptr: fuel_tx::TxPointer,
    /// Index of witness that authorizes spending the coin.
    #[clap(long)]
    pub witness_ix: u8,
    /// UTXO being spent must have been created at least this many blocks ago.
    #[clap(long)]
    pub maturity: u32,
    #[clap(flatten)]
    pub predicate: Predicate,
}

#[derive(Debug, Parser)]
pub struct InputContract {
    /// Hash of the unspent transaction.
    #[clap(long)]
    pub utxo_id: fuel_tx::UtxoId,
    /// Index of transaction output.
    #[clap(long)]
    pub output_ix: u8,
    /// Root of the amount of coins owned by the contract before transaction execution.
    #[clap(long)]
    pub balance_root: fuel_tx::Bytes32,
    /// State root of contract before transaction execution.
    #[clap(long)]
    pub state_root: fuel_tx::Bytes32,
    /// Points to the TX whose output is being spent. Includes block height, tx index.
    #[clap(long)]
    pub tx_ptr: fuel_tx::TxPointer,
    /// The ID of the contract.
    #[clap(long)]
    pub contract_id: fuel_tx::ContractId,
}

#[derive(Debug, Parser)]
pub struct InputMessage {
    /// The message ID as described here.
    #[clap(long)]
    pub msg_id: fuel_tx::MessageId,
    /// The address of the message sender.
    #[clap(long)]
    pub sender: fuel_tx::Address,
    /// The address or predicate root of the message recipient.
    #[clap(long)]
    pub recipient: fuel_tx::Address,
    /// Amount of base asset coins sent with message.
    #[clap(long)]
    pub amount: u64,
    /// The message nonce.
    #[clap(long)]
    pub nonce: u64,
    /// Index of witness that authorizes the message.
    #[clap(long)]
    pub witness_ix: u8,
    /// The message data.
    #[clap(long)]
    pub data: PathBuf,
    #[clap(flatten)]
    pub predicate: Predicate,
}

/// Grouped arguments related to an input's predicate.
#[derive(Debug, Parser)]
pub struct Predicate {
    /// The predicate bytecode.
    #[clap(long = "predicate")]
    pub bytecode: PathBuf,
    /// The predicate's input data (parameters). Specified file is loaded as raw bytes.
    #[clap(long = "predicate-data")]
    pub data: PathBuf,
}

/// The location of the transaction in the block.
#[derive(Debug, Parser)]
pub struct TxPointer {
    /// The transaction block height.
    #[clap(long = "tx-ptr-block-height")]
    pub block_height: u32,
    /// Transaction index.
    #[clap(long = "tx-ptr-ix")]
    pub tx_ix: u16,
}
