import { useEffect, useState } from 'react';
import { completeSetupWizard, getRuntimeReport, type RuntimeReport } from '@/services/setupService';
import { AudioWaveform, Layers, Lightbulb } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';

import { WizardHeader } from '../components/WizardHeader';
import { WizardFooter } from '../components/WizardFooter';
import { StatusCard } from '../components/StatusCard';
import { WIZARD_STEP_HEADERS, COMPLETED_STATUS_CARDS, COMPLETED_TIP, WIZARD_CTA_LABELS } from '@/data/welcomeCopy';

interface Props {
  onBack: () => void;
}

export const CompletedStep: React.FC<Props> = ({ onBack }) => {
  const [report, setReport] = useState<RuntimeReport | null>(null);

  useEffect(() => {
    let isMounted = true;
    getRuntimeReport()
      .then((r) => {
        if (isMounted) setReport(r);
      })
      .catch(() => {});
    return () => {
      isMounted = false;
    };
  }, []);

  const handleFinish = async () => {
    await completeSetupWizard();
  };

  // ENGINE and MODELS reflect the live runtime report; TRAY and PRIVACY are
  // static product capabilities (no tray-liveness probe exists), never claims.
  const okFor = (Icon: LucideIcon): boolean => {
    if (!report) return false;
    if (Icon === Layers) return report.models_verified;
    if (Icon === AudioWaveform) return report.mic_access && report.models_verified;
    return true;
  };

  return (
    <div className="flex flex-col h-full max-h-[100vh] overflow-y-auto lg:overflow-hidden justify-between relative select-none">
      <WizardHeader
        step={WIZARD_STEP_HEADERS.completed.step}
        title={WIZARD_STEP_HEADERS.completed.title}
        description={WIZARD_STEP_HEADERS.completed.description}
      />

      <div className="flex-1 flex flex-col gap-4 min-h-0 overflow-visible lg:overflow-hidden justify-center">
        {/* Harmonized Diagnostics Grid */}
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 shrink-0">
          {COMPLETED_STATUS_CARDS.map((card) => (
            <StatusCard
              key={card.label}
              icon={<card.icon className="w-4 h-4" />}
              label={card.label}
              value={card.value}
              subValue={card.subValue}
              ok={okFor(card.icon)}
              loading={report === null}
            />
          ))}
        </div>

        {/* Tip Card */}
        <div className="p-4 glass relative overflow-hidden shrink-0">
          <div className="absolute inset-0 bg-gradient-to-r from-[rgb(var(--accent))]/5 to-transparent opacity-50 pointer-events-none" />
          <div className="flex items-center gap-2 mb-2 relative z-10">
            <Lightbulb className="w-3 h-3 text-[rgb(var(--accent))]" />
            <span className="text-[12px] font-black text-[rgb(var(--accent))] uppercase tracking-[0.3em]">{COMPLETED_TIP.title}</span>
          </div>
          <p className="text-[12px] text-[rgb(var(--foreground-muted))]/80 leading-relaxed relative z-10 font-medium">
            {COMPLETED_TIP.text}
          </p>
        </div>
      </div>

      <WizardFooter 
        onBack={onBack}
        onNext={handleFinish}
        nextLabel={WIZARD_CTA_LABELS.startUsingVox}
        showBack={true}
        showSkip={false}
        className="mt-4 shrink-0"
      />
    </div>
  );
};
