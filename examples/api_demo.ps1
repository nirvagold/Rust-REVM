# Ruster REVM Cloud API Demo (PowerShell)
# Run: cargo run --bin ruster_api
# Then execute this script

$BASE_URL = "http://localhost:3000/v1"

Write-Host "=== Ruster REVM API Demo ===" -ForegroundColor Cyan
Write-Host ""

# Health check
Write-Host "1. Health Check:" -ForegroundColor Yellow
$health = Invoke-RestMethod -Uri "$BASE_URL/health" -Method Get
$health | ConvertTo-Json -Depth 5
Write-Host ""

# Honeypot check (USDT - should be safe)
Write-Host "2. Honeypot Check (USDT):" -ForegroundColor Yellow
$body = @{
    token_address = "0xdAC17F958D2ee523a2206206994597C13D831ec7"
    test_amount_eth = "0.1"
} | ConvertTo-Json

$honeypot = Invoke-RestMethod -Uri "$BASE_URL/honeypot/check" -Method Post -Body $body -ContentType "application/json"
$honeypot | ConvertTo-Json -Depth 5
Write-Host ""

# Full token analysis
Write-Host "3. Full PERS Analysis (DAI):" -ForegroundColor Yellow
$body = @{
    token_address = "0x6B175474E89094C44Da98b954EescdeCB5f8F4"
    test_amount_eth = "0.1"
    chain_id = 1
} | ConvertTo-Json

$analysis = Invoke-RestMethod -Uri "$BASE_URL/analyze/token" -Method Post -Body $body -ContentType "application/json"
$analysis | ConvertTo-Json -Depth 5
Write-Host ""

# Batch analysis
Write-Host "4. Batch Analysis (3 tokens):" -ForegroundColor Yellow
$body = @{
    tokens = @(
        "0xdAC17F958D2ee523a2206206994597C13D831ec7",
        "0x6B175474E89094C44Da98b954EescdeCB5f8F4",
        "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
    )
    test_amount_eth = "0.1"
    concurrency = 3
} | ConvertTo-Json

$batch = Invoke-RestMethod -Uri "$BASE_URL/analyze/batch" -Method Post -Body $body -ContentType "application/json"
$batch | ConvertTo-Json -Depth 5
Write-Host ""

# Stats
Write-Host "5. Protection Stats:" -ForegroundColor Yellow
$stats = Invoke-RestMethod -Uri "$BASE_URL/stats" -Method Get
$stats | ConvertTo-Json -Depth 5
Write-Host ""

Write-Host "=== Demo Complete ===" -ForegroundColor Cyan
