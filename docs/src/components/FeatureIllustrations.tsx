import React from 'react';

const primaryColor = "#5227FF";
const secondaryColor = "rgba(255, 255, 255, 0.2)";
const whiteColor = "rgba(255, 255, 255, 0.9)";

export const TestingIllustration = () => (
  <svg viewBox="0 0 400 300" className="w-full h-full" fill="none" xmlns="http://www.w3.org/2000/svg">
    <rect x="50" y="50" width="300" height="200" rx="8" stroke={secondaryColor} strokeWidth="2" fill="rgba(0,0,0,0.5)"/>
    {/* Header */}
    <rect x="50" y="50" width="300" height="40" rx="8" fill={secondaryColor} fillOpacity="0.1"/>
    <circle cx="70" cy="70" r="4" fill="#FF5F56"/>
    <circle cx="85" cy="70" r="4" fill="#FFBD2E"/>
    <circle cx="100" cy="70" r="4" fill="#27C93F"/>

    {/* Content */}
    <rect x="80" y="110" width="120" height="10" rx="2" fill={whiteColor} fillOpacity="0.5"/>
    <rect x="80" y="130" width="180" height="10" rx="2" fill={whiteColor} fillOpacity="0.3"/>

    <rect x="80" y="160" width="100" height="10" rx="2" fill={whiteColor} fillOpacity="0.5"/>
    <rect x="80" y="180" width="140" height="10" rx="2" fill={whiteColor} fillOpacity="0.3"/>

    {/* Checkmarks */}
    <path d="M280 115 L290 125 L310 105" stroke={primaryColor} strokeWidth="3" strokeLinecap="round"
          strokeLinejoin="round"/>
    <path d="M280 165 L290 175 L310 155" stroke={primaryColor} strokeWidth="3" strokeLinecap="round"
          strokeLinejoin="round"/>

    {/* Status Bar */}
    <rect x="50" y="230" width="300" height="20" fill={primaryColor} fillOpacity="0.1"/>
    <text x="70" y="244" fill={primaryColor} fontSize="12" fontFamily="monospace">PASS</text>
    <text x="280" y="244" fill={whiteColor} fillOpacity="0.5" fontSize="12" fontFamily="monospace">2/2</text>
  </svg>
);

export const CompilationIllustration = () => (
  <svg viewBox="0 0 400 300" className="w-full h-full" fill="none" xmlns="http://www.w3.org/2000/svg">
    {/* Input File */}
    <path d="M60 80 H140 V220 H60 V80 Z" stroke={secondaryColor} strokeWidth="2" fill="rgba(0,0,0,0.5)"/>
    <rect x="75" y="100" width="50" height="4" rx="2" fill={whiteColor} fillOpacity="0.5"/>
    <rect x="75" y="115" width="30" height="4" rx="2" fill={whiteColor} fillOpacity="0.3"/>
    <rect x="75" y="130" width="40" height="4" rx="2" fill={whiteColor} fillOpacity="0.3"/>

    {/* Arrow */}
    <path d="M160 150 H240" stroke={primaryColor} strokeWidth="2" strokeDasharray="8 8"/>
    <path d="M230 140 L240 150 L230 160" stroke={primaryColor} strokeWidth="2"/>

    {/* Gear */}
    <circle cx="200" cy="150" r="20" stroke={whiteColor} strokeOpacity="0.2" strokeWidth="2"/>
    <circle cx="200" cy="150" r="8" fill={primaryColor}/>

    {/* Output File - Hex/Binary */}
    <path d="M260 80 H340 V220 H260 V80 Z" stroke={secondaryColor} strokeWidth="2" fill="rgba(0,0,0,0.5)"/>
    <text x="275" y="110" fill={whiteColor} fillOpacity="0.5" fontSize="10" fontFamily="monospace">10110</text>
    <text x="275" y="125" fill={whiteColor} fillOpacity="0.5" fontSize="10" fontFamily="monospace">01001</text>
    <text x="275" y="140" fill={whiteColor} fillOpacity="0.5" fontSize="10" fontFamily="monospace">11100</text>
    <rect x="275" y="160" width="30" height="30" stroke={primaryColor} strokeWidth="1" fillOpacity="0.1"
          fill={primaryColor}/>
  </svg>
);

export const FormattingIllustration = () => (
  <svg viewBox="0 0 400 300" className="w-full h-full" fill="none" xmlns="http://www.w3.org/2000/svg">
    {/* Messy Code */}
    <g opacity="0.4">
      <rect x="60" y="60" width="80" height="8" rx="2" fill={whiteColor}/>
      <rect x="80" y="80" width="100" height="8" rx="2" fill={whiteColor}/>
      <rect x="50" y="100" width="60" height="8" rx="2" fill={whiteColor}/>
      <rect x="90" y="120" width="70" height="8" rx="2" fill={whiteColor}/>
    </g>

    {/* Transition */}
    <path d="M180 150 L220 150" stroke={primaryColor} strokeWidth="2" markerEnd="url(#arrow)"/>

    {/* Clean Code */}
    <g>
      <rect x="240" y="60" width="80" height="8" rx="2" fill={whiteColor}/>
      <rect x="260" y="80" width="100" height="8" rx="2" fill={whiteColor}/>
      <rect x="260" y="100" width="60" height="8" rx="2" fill={whiteColor}/>
      <rect x="260" y="120" width="70" height="8" rx="2" fill={whiteColor}/>

      {/* Alignment Guides */}
      <line x1="255" y1="50" x2="255" y2="140" stroke={primaryColor} strokeWidth="1" strokeDasharray="4 4"
            opacity="0.6"/>
    </g>

    {/* Brush/Tool Icon */}
    <circle cx="200" cy="220" r="30" fill="rgba(255,255,255,0.05)"/>
    <path d="M190 230 L210 210 M190 210 L210 230" stroke={primaryColor} strokeWidth="2"/>
  </svg>
);

export const DependencyIllustration = () => (
  <svg viewBox="0 0 400 300" className="w-full h-full" fill="none" xmlns="http://www.w3.org/2000/svg">
    {/* Central Node */}
    <circle cx="200" cy="150" r="30" fill={primaryColor} fillOpacity="0.2" stroke={primaryColor} strokeWidth="2"/>
    <circle cx="200" cy="150" r="10" fill={whiteColor}/>

    {/* Connected Nodes */}
    <circle cx="100" cy="80" r="20" stroke={secondaryColor} strokeWidth="2" fill="rgba(0,0,0,0.5)"/>
    <circle cx="300" cy="80" r="20" stroke={secondaryColor} strokeWidth="2" fill="rgba(0,0,0,0.5)"/>
    <circle cx="100" cy="220" r="20" stroke={secondaryColor} strokeWidth="2" fill="rgba(0,0,0,0.5)"/>
    <circle cx="300" cy="220" r="20" stroke={secondaryColor} strokeWidth="2" fill="rgba(0,0,0,0.5)"/>

    {/* Connections */}
    <path d="M180 135 L115 90" stroke={secondaryColor} strokeWidth="1"/>
    <path d="M220 135 L285 90" stroke={secondaryColor} strokeWidth="1"/>
    <path d="M180 165 L115 210" stroke={secondaryColor} strokeWidth="1"/>
    <path d="M220 165 L285 210" stroke={secondaryColor} strokeWidth="1"/>

    {/* Active Data Flow */}
    <circle cx="147.5" cy="112.5" r="3" fill={primaryColor}>
      <animate attributeName="opacity" values="0;1;0" dur="2s" repeatCount="indefinite"/>
    </circle>
  </svg>
);

export const ScriptIllustration = () => (
  <svg viewBox="0 0 400 300" className="w-full h-full" fill="none" xmlns="http://www.w3.org/2000/svg">
    <rect x="50" y="60" width="300" height="180" rx="8" stroke={secondaryColor} strokeWidth="2" fill="rgba(0,0,0,0.8)"/>

    <text x="70" y="100" fill={primaryColor} fontSize="14" fontFamily="monospace">$</text>
    <text x="90" y="100" fill={whiteColor} fontSize="14" fontFamily="monospace">acton script deploy.tolk</text>

    <text x="70" y="130" fill={whiteColor} fillOpacity="0.6" fontSize="12" fontFamily="monospace">Compiling...</text>
    <text x="70" y="150" fill={whiteColor} fillOpacity="0.6" fontSize="12" fontFamily="monospace">Running...</text>

    <rect x="70" y="170" width="10" height="18" fill={primaryColor}>
      <animate attributeName="opacity" values="0;1;0" dur="1s" repeatCount="indefinite"/>
    </rect>
  </svg>
);

export const VerificationIllustration = () => (
  <svg viewBox="0 0 400 300" className="w-full h-full" fill="none" xmlns="http://www.w3.org/2000/svg">
    {/* Contract Document */}
    <path d="M120 60 H230 L280 110 V240 H120 V60 Z" stroke={secondaryColor} strokeWidth="2" fill="rgba(0,0,0,0.5)"/>
    <path d="M230 60 V110 H280" stroke={secondaryColor} strokeWidth="2"/>

    {/* Code Lines */}
    <rect x="140" y="100" width="80" height="6" rx="3" fill={whiteColor} fillOpacity="0.3"/>
    <rect x="140" y="120" width="100" height="6" rx="3" fill={whiteColor} fillOpacity="0.3"/>
    <rect x="140" y="140" width="60" height="6" rx="3" fill={whiteColor} fillOpacity="0.3"/>

    {/* Shield/Verify Badge */}
    <circle cx="260" cy="200" r="40" fill="#000" stroke={primaryColor} strokeWidth="3"/>
    <path d="M245 200 L255 210 L275 190" stroke={whiteColor} strokeWidth="4" strokeLinecap="round"
          strokeLinejoin="round"/>
  </svg>
);

export const BlockchainIllustration = () => (
  <svg viewBox="0 0 400 300" className="w-full h-full" fill="none" xmlns="http://www.w3.org/2000/svg">
    {/* Blocks */}
    <rect x="60" y="120" width="60" height="60" rx="4" stroke={secondaryColor} strokeWidth="2" fill="rgba(0,0,0,0.5)"/>
    <rect x="170" y="120" width="60" height="60" rx="4" stroke={secondaryColor} strokeWidth="2" fill="rgba(0,0,0,0.5)"/>
    <rect x="280" y="120" width="60" height="60" rx="4" stroke={primaryColor} strokeWidth="2"
          fill="rgba(82, 39, 255, 0.1)"/>

    {/* Chains */}
    <path d="M120 150 H170" stroke={whiteColor} strokeOpacity="0.3" strokeWidth="2"/>
    <path d="M230 150 H280" stroke={whiteColor} strokeOpacity="0.3" strokeWidth="2"/>

    {/* Network Effect */}
    <circle cx="90" cy="150" r="4" fill={whiteColor} fillOpacity="0.5"/>
    <circle cx="200" cy="150" r="4" fill={whiteColor} fillOpacity="0.5"/>
    <circle cx="310" cy="150" r="4" fill={primaryColor}/>

    {/* Floating Elements */}
    <circle cx="320" cy="100" r="2" fill={primaryColor} opacity="0.6"/>
    <circle cx="290" cy="190" r="3" fill={primaryColor} opacity="0.4"/>
  </svg>
);

export const getIllustrationForFeature = (index: number) => {
  switch (index) {
    case 0:
      return <TestingIllustration/>;
    case 1:
      return <CompilationIllustration/>;
    case 2:
      return <FormattingIllustration/>;
    case 3:
      return <DependencyIllustration/>;
    case 4:
      return <ScriptIllustration/>;
    case 5:
      return <VerificationIllustration/>;
    case 6:
      return <BlockchainIllustration/>;
    default:
      return null;
  }
};
