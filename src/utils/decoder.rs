//! Transaction decoder module
//! Parses DEX swap calldata to extract swap parameters

use crate::models::types::SwapParams;
use alloy_primitives::{Bytes, U256};
use alloy_sol_types::{sol, SolCall};

// Uniswap V2 Router function signatures
sol! {
    function swapExactETHForTokens(
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external payable returns (uint256[] memory amounts);

    function swapExactTokensForETH(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory amounts);

    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory amounts);

    function swapETHForExactTokens(
        uint256 amountOut,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external payable returns (uint256[] memory amounts);

    function swapExactTokensForETHSupportingFeeOnTransferTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external;

    function swapExactETHForTokensSupportingFeeOnTransferTokens(
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external payable;

    function swapExactTokensForTokensSupportingFeeOnTransferTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external;
}

/// Decoder for DEX swap transactions
pub struct SwapDecoder;

impl SwapDecoder {
    /// Decode swap parameters from transaction calldata
    /// Returns None if not a recognized swap function
    pub fn decode(calldata: &Bytes, value: U256) -> Option<SwapParams> {
        if calldata.len() < 4 {
            return None;
        }

        let _selector = &calldata[..4];
        let data = calldata.as_ref();

        // Try each swap function signature
        Self::try_decode_swap_exact_eth_for_tokens(data, value)
            .or_else(|| Self::try_decode_swap_exact_tokens_for_eth(data))
            .or_else(|| Self::try_decode_swap_exact_tokens_for_tokens(data))
            .or_else(|| Self::try_decode_swap_eth_for_exact_tokens(data, value))
            .or_else(|| Self::try_decode_fee_on_transfer_eth_for_tokens(data, value))
            .or_else(|| Self::try_decode_fee_on_transfer_tokens_for_eth(data))
            .or_else(|| Self::try_decode_fee_on_transfer_tokens_for_tokens(data))
    }

    fn try_decode_swap_exact_eth_for_tokens(data: &[u8], value: U256) -> Option<SwapParams> {
        let call = swapExactETHForTokensCall::abi_decode(data, false).ok()?;
        Some(SwapParams {
            amount_in: value,
            amount_out_min: call.amountOutMin,
            path: call.path,
            deadline: call.deadline,
        })
    }

    fn try_decode_swap_exact_tokens_for_eth(data: &[u8]) -> Option<SwapParams> {
        let call = swapExactTokensForETHCall::abi_decode(data, false).ok()?;
        Some(SwapParams {
            amount_in: call.amountIn,
            amount_out_min: call.amountOutMin,
            path: call.path,
            deadline: call.deadline,
        })
    }

    fn try_decode_swap_exact_tokens_for_tokens(data: &[u8]) -> Option<SwapParams> {
        let call = swapExactTokensForTokensCall::abi_decode(data, false).ok()?;
        Some(SwapParams {
            amount_in: call.amountIn,
            amount_out_min: call.amountOutMin,
            path: call.path,
            deadline: call.deadline,
        })
    }

    fn try_decode_swap_eth_for_exact_tokens(data: &[u8], value: U256) -> Option<SwapParams> {
        let call = swapETHForExactTokensCall::abi_decode(data, false).ok()?;
        Some(SwapParams {
            amount_in: value,
            amount_out_min: call.amountOut,
            path: call.path,
            deadline: call.deadline,
        })
    }

    fn try_decode_fee_on_transfer_eth_for_tokens(data: &[u8], value: U256) -> Option<SwapParams> {
        let call =
            swapExactETHForTokensSupportingFeeOnTransferTokensCall::abi_decode(data, false).ok()?;
        Some(SwapParams {
            amount_in: value,
            amount_out_min: call.amountOutMin,
            path: call.path,
            deadline: call.deadline,
        })
    }

    fn try_decode_fee_on_transfer_tokens_for_eth(data: &[u8]) -> Option<SwapParams> {
        let call =
            swapExactTokensForETHSupportingFeeOnTransferTokensCall::abi_decode(data, false).ok()?;
        Some(SwapParams {
            amount_in: call.amountIn,
            amount_out_min: call.amountOutMin,
            path: call.path,
            deadline: call.deadline,
        })
    }

    fn try_decode_fee_on_transfer_tokens_for_tokens(data: &[u8]) -> Option<SwapParams> {
        let call =
            swapExactTokensForTokensSupportingFeeOnTransferTokensCall::abi_decode(data, false)
                .ok()?;
        Some(SwapParams {
            amount_in: call.amountIn,
            amount_out_min: call.amountOutMin,
            path: call.path,
            deadline: call.deadline,
        })
    }

    /// Calculate implied slippage from swap params (in basis points)
    #[allow(dead_code)]
    pub fn calculate_slippage_bps(
        amount_in: U256,
        amount_out_min: U256,
        expected_rate: U256,
    ) -> u64 {
        if expected_rate.is_zero() || amount_in.is_zero() {
            return 0;
        }

        // expected_out = amount_in * expected_rate
        // slippage = (expected_out - amount_out_min) / expected_out * 10000
        let expected_out = amount_in.saturating_mul(expected_rate);
        if expected_out <= amount_out_min {
            return 0;
        }

        let diff = expected_out.saturating_sub(amount_out_min);
        // Convert to basis points (multiply by 10000)
        let slippage = diff.saturating_mul(U256::from(10000)) / expected_out;

        // Safe conversion - slippage should never exceed 10000 bps (100%)
        slippage.try_into().unwrap_or(10000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slippage_calculation() {
        let amount_in = U256::from(1_000_000_000_000_000_000u128); // 1 ETH
        let amount_out_min = U256::from(970_000_000_000_000_000u128); // 0.97 ETH equivalent
        let expected_rate = U256::from(1); // 1:1 rate

        let slippage =
            SwapDecoder::calculate_slippage_bps(amount_in, amount_out_min, expected_rate);
        assert_eq!(slippage, 300); // 3% slippage
    }
}
