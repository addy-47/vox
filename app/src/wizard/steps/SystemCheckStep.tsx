import React, { useEffect, useState } from 'react';
import { HardDrive, Cpu, Mic, ShieldCheck } from 'lucide-react';
import { motion } from 'framer-motion';
import { getRuntimeReport, type RuntimeReport } from '@/services/modelService';

import { WizardHeader } from '../components/WizardHeader';
import { WizardFooter } from '../components/WizardFooter';
import { StatusCard } from '../components/StatusCard';
import { WIZARD_STEP_HEADERS, SYSTEM_CHECK_LABELS, WIZARD_CTA_LABELS } from '@/data/welcomeCopy';

interface Props {
  onNext: () => void;
  onBack: () => void;
  error?: string;
}

export const SystemCheckStep: React.FC<Props> = ({ onNext, onBack, error: externalError }) => {
  const [report, setReport] = useState<RuntimeReport | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const check = async () => {
      setIsLoading(true);
      try {
        // fetch_manifest is already called in WizardRoot, avoiding redundant IPC call here
        const r = await getRuntimeReport();
        setReport(r);
      } catch (e) {
        console.error('System check failed', e);
      } finally {
        setIsLoading(false);
      }
    };
    check();
  }, []);

  const systemOk = Boolean(report && report.write_access && report.disk_space_ok);
  const allOk = Boolean(systemOk && report?.mic_access);
  const micMissingOnly = Boolean(systemOk && !report?.mic_access);

  return (
    <div className="flex flex-col h-full relative">
      <WizardHeader
        step={WIZARD_STEP_HEADERS.checking.step}
        title={WIZARD_STEP_HEADERS.checking.title}
        description={WIZARD_STEP_HEADERS.checking.description}
      />

      <div className="flex-1 overflow-y-auto pr-2 custom-scrollbar">
        <div className="grid grid-cols-2 gap-4">
            <motion.div
                key="disk"
                initial={{ opacity: 0, y: 5 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.1 }}
            >
                <StatusCard 
                    icon={<HardDrive className="w-4 h-4" />}
                    label={SYSTEM_CHECK_LABELS[0]}
                    value={report ? (report.disk_space_ok ? `${report.available_space_gb.toFixed(1)} GB` : "INSUFFICIENT") : "Checking..."}
                    subValue={report ? (report.disk_space_ok ? "Sufficient for neural models" : "At least 10GB recommended") : "Measuring available space..."}
                    ok={report?.disk_space_ok}
                    loading={isLoading}
                />
            </motion.div>
            <motion.div
                key="mic"
                initial={{ opacity: 0, y: 5 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.15 }}
            >
                <StatusCard 
                    icon={<Mic className="w-4 h-4" />}
                    label={SYSTEM_CHECK_LABELS[1]}
                    value={report ? (report.mic_access ? "DETECTED" : "NOT FOUND") : "Checking..."}
                    subValue={report ? (report.mic_access ? "Audio input available" : "No capture device found") : "Testing audio devices..."}
                    ok={report?.mic_access}
                    loading={isLoading}
                />
            </motion.div>
            <motion.div
                key="write"
                initial={{ opacity: 0, y: 5 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.2 }}
            >
                <StatusCard 
                    icon={<ShieldCheck className="w-4 h-4" />}
                    label={SYSTEM_CHECK_LABELS[2]}
                    value={report ? (report.write_access ? "GRANTED" : "DENIED") : "Checking..."}
                    subValue={report ? (report.write_access ? "Sandbox I/O verified" : "Check folder access") : "Verifying access..."}
                    ok={report?.write_access}
                    loading={isLoading}
                />
            </motion.div>
            <motion.div
                key="hardware"
                initial={{ opacity: 0, y: 5 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.25 }}
            >
                <StatusCard 
                    icon={<Cpu className="w-4 h-4" />}
                    label={SYSTEM_CHECK_LABELS[3]}
                    value={report ? `${report.cpu_cores} THREADS` : "Scanning..."}
                    subValue={report ? `${report.ram_gb.toFixed(1)} GB RAM detected` : "Detecting memory..."}
                    ok={true}
                    loading={isLoading}
                />
            </motion.div>
        </div>
      </div>

      <WizardFooter 
        onBack={onBack}
        onNext={onNext}
        onSkip={micMissingOnly ? onNext : undefined}
        showSkip={micMissingOnly}
        nextLabel={WIZARD_CTA_LABELS.continueToModels}
        isNextDisabled={!allOk || isLoading}
        showBack={true}
        error={
          externalError ||
          (!systemOk && !isLoading
            ? "Storage or folder permissions need attention"
            : micMissingOnly
            ? "No microphone detected. You can proceed and configure audio later."
            : undefined)
        }
        errorLabel={micMissingOnly ? "Microphone Warning" : "Setup Check Failed"}
      />
    </div>
  );
};
