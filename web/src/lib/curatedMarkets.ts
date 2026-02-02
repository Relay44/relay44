export interface CuratedMarketDefinition {
  question: string;
  outcomes: string;
  rationale: string;
  category: string;
}

export const CURATED_MARKETS_BY_ID: Record<number, CuratedMarketDefinition> = {
  1: {
    question: 'Which crypto exchange will ZachXBT investigate next for insider trading by June 2026?',
    outcomes: 'Axiom | Meteora | Polymarket | Coinbase | None',
    rationale:
      'Recent speculation on X and investigations into Axiom staff abusing user data for trading.',
    category: 'Crypto',
  },
  2: {
    question:
      'Will Coffeezilla expose a major AI deepfake scam involving a celebrity endorsement by April 2026?',
    outcomes: 'Yes | No',
    rationale:
      'AI deepfake scams surged 20% in 2025, with tactics like fake influencer videos promising giveaways.',
    category: 'Tech & Science',
  },
  3: {
    question: 'Which DeFi protocol will be the next rug pull target exposed by on-chain sleuths like Spreekaway?',
    outcomes: 'Specific protocol | None by date',
    rationale:
      'DeFi rug pulls accounted for $6B losses in early 2026, with 80% in memecoins.',
    category: 'Crypto',
  },
  4: {
    question: 'Will ZachXBT link a major 2026 crypto hack to a privacy coin like Monero before year-end?',
    outcomes: 'Yes | No',
    rationale:
      "ZachXBT's past work tracing hacks to privacy pools, amid $400M in January 2026 thefts.",
    category: 'Crypto',
  },
  5: {
    question: 'Which tech giant will face an EU antitrust investigation for AI practices next?',
    outcomes: 'Meta | Microsoft | ByteDance | None by mid-2026',
    rationale:
      'Ongoing probes into AI tools like Grok for sexualized images and facial recognition privacy issues.',
    category: 'Companies',
  },
  6: {
    question: 'Will a whistleblower like Nick Bax reveal quantum attack vulnerabilities in a major bank by September 2026?',
    outcomes: 'Yes | No',
    rationale:
      'Reports warn of $3.3T potential losses from quantum threats, with rising regulatory pressure.',
    category: 'Financials',
  },
  7: {
    question: 'Which politician implicated in the Epstein files will be arrested next?',
    outcomes: 'Named figure | None by date',
    rationale:
      'Recent arrests like Peter Mandelson and fallout from Epstein documents causing political reckonings.',
    category: 'Politics',
  },
  8: {
    question: 'Will JP on Chain expose a pig butchering scam tied to a Cambodian compound in 2026?',
    outcomes: 'Yes | No',
    rationale:
      'Pig butchering schemes stole $17B in 2025, often run from Southeast Asian fraud operations.',
    category: 'Crypto',
  },
  9: {
    question: 'Which crypto ATM operator will be sued for facilitating scams next?',
    outcomes: 'Bitcoin Depot | Other operator | None',
    rationale:
      'New England states targeting operators amid $10M+ losses to kiosk-based fraud.',
    category: 'Crypto',
  },
  10: {
    question: 'Will Coffeezilla investigate a fake investment platform promising 10-50% returns by May 2026?',
    outcomes: 'Yes | No',
    rationale:
      'Investment scams dominated 62% of 2025 fraud, with AI enabling sophisticated fake sites.',
    category: 'Financials',
  },
  11: {
    question: 'Which company will ZachXBT probe for phishing attacks targeting hardware wallets next?',
    outcomes: 'Ledger | Other company | None',
    rationale:
      '$284M lost in a single January 2026 phishing attack on hardware wallets.',
    category: 'Crypto',
  },
  12: {
    question: 'Will a major media outlet like ProPublica expose Trump pardon-related corruption by election time?',
    outcomes: 'Yes | No',
    rationale:
      'Trump pardons for fraudsters (e.g., money laundering) under scrutiny, with calls for audits.',
    category: 'Politics',
  },
  13: {
    question: 'Which Solana project will be the next insider trading scandal uncovered by ZachXBT?',
    outcomes: 'Step Finance | Other project | None',
    rationale:
      "$30M exploit on Solana's Step Finance in January 2026, amid ecosystem controversies.",
    category: 'Crypto',
  },
  14: {
    question: 'Will Bellingcat investigate AI malware used in a cyberattack on critical infrastructure?',
    outcomes: 'Yes | No',
    rationale:
      'AI changing malware landscape, with agents finding 77% of software vulnerabilities.',
    category: 'Tech & Science',
  },
  15: {
    question: 'Which celebrity will be linked to an Epstein scandal in new file releases by July 2026?',
    outcomes: 'Named figure | None',
    rationale:
      'Epstein files triggering arrests and muted US response vs. European fallout.',
    category: 'Politics',
  },
  16: {
    question: 'Will ZachXBT expose a romance scam using AI voices/videos targeting seniors?',
    outcomes: 'Yes | No',
    rationale:
      'Romance scams with AI fakes up 20% in 2025, affecting 1 in 10 adults over 50.',
    category: 'Culture',
  },
  17: {
    question: 'Which tech firm will face a class-action over non-consensual AI imagery generation?',
    outcomes: 'xAI (Grok) | Other firm | None by date',
    rationale:
      'Grok controversy with 6,700 sexualized images per hour, sparking global scrutiny.',
    category: 'Tech & Science',
  },
  18: {
    question: 'Will a grand jury indict more Democrats in Trump administration probes by mid-2026?',
    outcomes: 'Yes | No',
    rationale:
      'Recent rejections of indictments over videos advising against illegal orders.',
    category: 'Politics',
  },
  19: {
    question: 'Which offshore exchange will be dismantled for scam facilitation next?',
    outcomes: 'TradeOgre | Other exchange | None',
    rationale:
      'RCMP action on TradeOgre for links to scams and darknet markets.',
    category: 'Crypto',
  },
  20: {
    question: 'Will Coffeezilla reveal a pyramid scheme disguised as a crypto investment club?',
    outcomes: 'Yes | No',
    rationale:
      'Pyramid/Ponzi schemes prominent in 2025 fraud, converging with investment tactics.',
    category: 'Financials',
  },
};
