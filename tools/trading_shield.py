#!/usr/bin/env python3
"""
RUSTER SHIELD - Trading Protection CLI
Honeypot & Risk Scanner untuk Token Crypto
"""

import requests
import re
import sys

try:
    from colorama import init, Fore, Style
    init(autoreset=True)
except ImportError:
    print("Installing colorama...")
    import subprocess
    subprocess.check_call([sys.executable, "-m", "pip", "install", "colorama", "-q"])
    from colorama import init, Fore, Style
    init(autoreset=True)

# API Configuration
API_URL = "http://yelling-patience-nirvagold-0a943e82.koyeb.app/v1/honeypot/check"

def print_banner():
    """Display ASCII banner"""
    banner = f"""
{Fore.CYAN}╔══════════════════════════════════════════════════════════════╗
║                                                              ║
║   ██████╗ ██╗   ██╗███████╗████████╗███████╗██████╗          ║
║   ██╔══██╗██║   ██║██╔════╝╚══██╔══╝██╔════╝██╔══██╗         ║
║   ██████╔╝██║   ██║███████╗   ██║   █████╗  ██████╔╝         ║
║   ██╔══██╗██║   ██║╚════██║   ██║   ██╔══╝  ██╔══██╗         ║
║   ██║  ██║╚██████╔╝███████║   ██║   ███████╗██║  ██║         ║
║   ╚═╝  ╚═╝ ╚═════╝ ╚══════╝   ╚═╝   ╚══════╝╚═╝  ╚═╝         ║
║                                                              ║
║            ███████╗██╗  ██╗██╗███████╗██╗     ██████╗        ║
║            ██╔════╝██║  ██║██║██╔════╝██║     ██╔══██╗       ║
║            ███████╗███████║██║█████╗  ██║     ██║  ██║       ║
║            ╚════██║██╔══██║██║██╔══╝  ██║     ██║  ██║       ║
║            ███████║██║  ██║██║███████╗███████╗██████╔╝       ║
║            ╚══════╝╚═╝  ╚═╝╚═╝╚══════╝╚══════╝╚═════╝        ║
║                                                              ║
║          {Fore.WHITE}🛡️  Trading Protection System  🛡️{Fore.CYAN}               ║
║              {Fore.YELLOW}Powered by PERS Algorithm{Fore.CYAN}                    ║
╚══════════════════════════════════════════════════════════════╝{Style.RESET_ALL}
"""
    print(banner)

def validate_address(address: str) -> bool:
    """Validate Ethereum address format"""
    pattern = r'^0x[a-fA-F0-9]{40}$'
    return bool(re.match(pattern, address))

def check_token(token_address: str) -> dict:
    """Call API to check token"""
    try:
        response = requests.post(
            API_URL,
            json={"token_address": token_address},
            headers={"Content-Type": "application/json"},
            timeout=30
        )
        response.raise_for_status()
        return response.json()
    except requests.exceptions.Timeout:
        return {"error": "Request timeout - server tidak merespons"}
    except requests.exceptions.ConnectionError:
        return {"error": "Tidak dapat terhubung ke server"}
    except requests.exceptions.HTTPError as e:
        return {"error": f"HTTP Error: {e.response.status_code}"}
    except Exception as e:
        return {"error": str(e)}

def display_result(result: dict, token_address: str):
    """Display analysis result with colors"""
    print(f"\n{Fore.CYAN}{'='*60}")
    print(f"📋 Token: {Fore.WHITE}{token_address}")
    print(f"{Fore.CYAN}{'='*60}{Style.RESET_ALL}\n")
    
    if "error" in result:
        print(f"{Fore.RED}❌ Error: {result['error']}{Style.RESET_ALL}")
        return
    
    # API returns data inside "data" object
    data = result.get("data", result)
    
    # Extract data with correct field names from API
    is_honeypot = data.get("is_honeypot", False)
    risk_score = data.get("risk_score", 0)
    buy_tax = data.get("buy_tax_percent", 0)
    sell_tax = data.get("sell_tax_percent", 0)
    total_loss = data.get("total_loss_percent", 0)
    buy_success = data.get("buy_success", True)
    sell_success = data.get("sell_success", True)
    reason = data.get("reason", "")
    
    # Decision logic - more accurate detection
    if is_honeypot or not sell_success:
        print(f"{Fore.RED}{'='*60}")
        print(f"{Fore.RED}🚨🚨🚨 JANGAN BELI! HONEYPOT TERDETEKSI 🚨🚨🚨")
        print(f"{Fore.RED}{'='*60}{Style.RESET_ALL}")
        print(f"\n{Fore.RED}Token ini adalah SCAM! Anda TIDAK akan bisa menjual!")
        status = "HONEYPOT"
    elif risk_score >= 70 or total_loss > 30:
        print(f"{Fore.RED}{'='*60}")
        print(f"{Fore.RED}🚨 RISIKO SANGAT TINGGI - JANGAN BELI! 🚨")
        print(f"{Fore.RED}{'='*60}{Style.RESET_ALL}")
        status = "CRITICAL"
    elif risk_score >= 50 or total_loss > 15:
        print(f"{Fore.YELLOW}{'='*60}")
        print(f"{Fore.YELLOW}⚠️  RISIKO TINGGI - HATI-HATI! ⚠️")
        print(f"{Fore.YELLOW}{'='*60}{Style.RESET_ALL}")
        status = "HIGH RISK"
    elif risk_score >= 30 or total_loss > 5:
        print(f"{Fore.YELLOW}{'='*60}")
        print(f"{Fore.YELLOW}⚠️  RISIKO SEDANG - PERHATIKAN TAX ⚠️")
        print(f"{Fore.YELLOW}{'='*60}{Style.RESET_ALL}")
        status = "MEDIUM RISK"
    else:
        print(f"{Fore.GREEN}{'='*60}")
        print(f"{Fore.GREEN}✅ RELATIF AMAN UNTUK TRADE ✅")
        print(f"{Fore.GREEN}{'='*60}{Style.RESET_ALL}")
        status = "SAFE"
    
    # Display details
    print(f"\n{Fore.CYAN}📊 Detail Analisis:{Style.RESET_ALL}")
    print(f"   • Status       : {get_status_color(status)}{status}{Style.RESET_ALL}")
    print(f"   • Risk Score   : {get_score_color(risk_score)}{risk_score}/100{Style.RESET_ALL}")
    print(f"   • Buy Success  : {Fore.GREEN if buy_success else Fore.RED}{'✓' if buy_success else '✗'}{Style.RESET_ALL}")
    print(f"   • Sell Success : {Fore.GREEN if sell_success else Fore.RED}{'✓' if sell_success else '✗'}{Style.RESET_ALL}")
    print(f"   • Buy Tax      : {get_tax_color(buy_tax)}{buy_tax:.2f}%{Style.RESET_ALL}")
    print(f"   • Sell Tax     : {get_tax_color(sell_tax)}{sell_tax:.2f}%{Style.RESET_ALL}")
    print(f"   • Total Loss   : {get_tax_color(total_loss)}{total_loss:.2f}%{Style.RESET_ALL}")
    
    if reason:
        print(f"\n{Fore.CYAN}📝 Reason:{Style.RESET_ALL}")
        print(f"   {reason}")
    
    # Warning messages based on findings
    warnings = []
    if not sell_success:
        warnings.append("🚫 SELL GAGAL - Token tidak bisa dijual!")
    if sell_tax > 50:
        warnings.append(f"💸 Sell tax sangat tinggi ({sell_tax:.1f}%)")
    if buy_tax > 20:
        warnings.append(f"💸 Buy tax tinggi ({buy_tax:.1f}%)")
    if total_loss > 20:
        warnings.append(f"📉 Total loss tinggi ({total_loss:.1f}%)")
    
    if warnings:
        print(f"\n{Fore.RED}⚠️  Peringatan:{Style.RESET_ALL}")
        for w in warnings:
            print(f"   {Fore.RED}• {w}{Style.RESET_ALL}")

def get_status_color(status: str) -> str:
    colors = {
        "HONEYPOT": Fore.RED, 
        "CRITICAL": Fore.RED,
        "HIGH RISK": Fore.YELLOW, 
        "MEDIUM RISK": Fore.YELLOW,
        "SAFE": Fore.GREEN
    }
    return colors.get(status, Fore.WHITE)

def get_score_color(score: int) -> str:
    if score > 70: return Fore.RED
    if score > 40: return Fore.YELLOW
    return Fore.GREEN

def get_tax_color(tax: float) -> str:
    if tax > 10: return Fore.RED
    if tax > 5: return Fore.YELLOW
    return Fore.GREEN

def main():
    """Main loop"""
    print_banner()
    print(f"{Fore.WHITE}Masukkan alamat token untuk mengecek keamanannya.")
    print(f"Ketik {Fore.YELLOW}'exit'{Fore.WHITE} atau {Fore.YELLOW}'quit'{Fore.WHITE} untuk keluar.\n")
    
    while True:
        try:
            token = input(f"{Fore.CYAN}🔍 Token Address: {Style.RESET_ALL}").strip()
            
            if token.lower() in ['exit', 'quit', 'q']:
                print(f"\n{Fore.CYAN}👋 Terima kasih telah menggunakan Ruster Shield!")
                print(f"   Stay safe, trade smart! 🛡️{Style.RESET_ALL}\n")
                break
            
            if not token:
                continue
            
            if not validate_address(token):
                print(f"{Fore.RED}❌ Format tidak valid! Alamat harus diawali '0x' dengan 42 karakter.{Style.RESET_ALL}")
                print(f"{Fore.YELLOW}   Contoh: 0xdAC17F958D2ee523a2206206994597C13D831ec7{Style.RESET_ALL}\n")
                continue
            
            print(f"\n{Fore.YELLOW}⏳ Menganalisis token...{Style.RESET_ALL}")
            result = check_token(token)
            display_result(result, token)
            print()
            
        except KeyboardInterrupt:
            print(f"\n\n{Fore.CYAN}👋 Goodbye! Stay safe! 🛡️{Style.RESET_ALL}\n")
            break

if __name__ == "__main__":
    main()
