import React from 'react';
import { completeSetupWizard } from '@/services/settingsService';
import { Zap, Check } from 'lucide-react';

// --- Modular Components ---
import { WizardHeader } from '../components/WizardHeader';
import { WizardFooter } from '../components/WizardFooter';
import { StatusCard } from '../components/StatusCard';

interface Props {
  onBack: () => void;
}

export const CompletedStep: React.FC<Props> = ({ onBack }) => {
  const handleFinish = async () => {
    await completeSetupWizard();
  };

  return (
    <div className="flex flex-col h-full max-h-[100vh] overflow-hidden justify-between relative select-none">
      <WizardHeader 
        step="Step 6 of 6 · All Done"
        title="Setup Complete."
        description="Vox is installed and ready to use."
      />

      <div className="flex-1 flex flex-col gap-4 min-h-0 overflow-hidden justify-center">
        {/* Harmonized Diagnostics Grid */}
        <div className="grid grid-cols-2 gap-3 shrink-0">
          <StatusCard 
            icon={<Check className="w-4 h-4" />}
            label="VOICE ENGINE"
            value="READY"
            subValue="Runs completely offline"
            ok={true}
          />
          <StatusCard 
            icon={<Check className="w-4 h-4" />}
            label="VOICE MODELS"
            value="READY"
            subValue="Configured on your device"
            ok={true}
          />
          <StatusCard 
            icon={<Check className="w-4 h-4" />}
            label="SYSTEM TRAY"
            value="RUNNING"
            subValue="Access from your menu bar"
            ok={true}
          />
          <StatusCard 
            icon={<Check className="w-4 h-4" />}
            label="PRIVACY"
            value="SECURED"
            subValue="100% private & safe"
            ok={true}
          />
        </div>

        {/* Tip Card */}
        <div className="p-4 glass relative overflow-hidden shrink-0">
          <div className="absolute inset-0 bg-gradient-to-r from-[rgb(var(--accent))]/5 to-transparent opacity-50 pointer-events-none" />
          <div className="flex items-center gap-2 mb-2 relative z-10">
            <Zap className="w-3 h-3 text-[rgb(var(--accent))]" />
            <span className="text-[12px] font-black text-[rgb(var(--accent))] uppercase tracking-[0.3em]">Quick Tip</span>
          </div>
          <p className="text-[12px] text-[rgb(var(--foreground-muted))]/80 leading-relaxed relative z-10 font-medium">
            Click the Vox icon in your menu bar or press your shortcut key to start talking.
          </p>
        </div>
      </div>

      <WizardFooter 
        onBack={onBack}
        onNext={handleFinish}
        nextLabel="Start Using Vox"
        showBack={true}
        showSkip={false}
        className="mt-4 shrink-0"
      />
    </div>
  );
};
