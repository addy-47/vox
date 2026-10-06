import React, { useEffect, useState } from 'react';
import { HardDrive, Microchip, Mic, ShieldCheck } from 'lucide-react';
import { motion } from 'framer-motion';
import { getRuntimeReport, type RuntimeReport } from '@/services/setupService';

import { WizardHeader } from '../components/WizardHeader';
import { WizardFooter } from '../components/WizardFooter';
import { StatusCard } from '../components/StatusCard';
import { WIZARD_STEP_HEADERS, SYSTEM_CHECK_LABELS, SYSTEM_CHECK_COPY, WIZARD_CTA_LABELS } from '@/data/welcomeCopy';

interface Props {
  onNext: () => void;
  onBack: () => void;
  error?: string;
}

export const SystemCheckStep: React.FC<Props> = ({ onNext, onBack, error: externalError }) => {
  const [report, setReport] = useState<RuntimeReport | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [checkError, setCheckError] = useState<string | null>(null);

  useEffect(() => {
    const check = async () => {
      setIsLoading(true);
      setCheckError(null);
      try {
        // fetch_manifest is already called in WizardRoot, avoiding redundant IPC call here
        const r = await getRuntimeReport();
        setReport(r);
      } catch (e) {
        console.error('System check failed', e);
        setCheckError(e instanceof Error ? e.message : String(e));
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
                    value={report ? (report.disk_space_unknown ? SYSTEM_CHECK_COPY.diskUnknown : report.disk_space_ok ? `${report.available_space_gb.toFixed(1)} GB` : SYSTEM_CHECK_COPY.insufficient) : SYSTEM_CHECK_COPY.checkingValue}
                    subValue={report ? (report.disk_space_unknown ? SYSTEM_CHECK_COPY.diskUnknownSub : report.disk_space_ok ? SYSTEM_CHECK_COPY.diskOkSub : SYSTEM_CHECK_COPY.tenGbNote) : SYSTEM_CHECK_COPY.measuringSub}
                    ok={report ? (report.disk_space_unknown ? undefined : report.disk_space_ok) : undefined}
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
                    value={report ? (report.mic_access ? SYSTEM_CHECK_COPY.detected : SYSTEM_CHECK_COPY.missing) : SYSTEM_CHECK_COPY.checkingValue}
                    subValue={report ? (report.mic_access ? SYSTEM_CHECK_COPY.micOkSub : SYSTEM_CHECK_COPY.micMissingSub) : SYSTEM_CHECK_COPY.testingSub}
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
                    value={report ? (report.write_access ? SYSTEM_CHECK_COPY.granted : SYSTEM_CHECK_COPY.denied) : SYSTEM_CHECK_COPY.checkingValue}
                    subValue={report ? (report.write_access ? SYSTEM_CHECK_COPY.writeOkSub : SYSTEM_CHECK_COPY.writeDeniedSub) : SYSTEM_CHECK_COPY.verifyingSub}
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
                    icon={<Microchip className="w-4 h-4" />}
                    label={SYSTEM_CHECK_LABELS[3]}
                    value={report ? `${report.cpu_cores} ${SYSTEM_CHECK_COPY.threadsSuffix}` : SYSTEM_CHECK_COPY.checkingValue}
                    subValue={report ? `${report.ram_gb.toFixed(1)} ${SYSTEM_CHECK_COPY.ramDetected}` : SYSTEM_CHECK_COPY.scanningSub}
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
          checkError ||
          (!systemOk && !isLoading
            ? SYSTEM_CHECK_COPY.storagePermsError
            : micMissingOnly
            ? SYSTEM_CHECK_COPY.micWarnError
            : undefined)
        }
        errorLabel={micMissingOnly ? SYSTEM_CHECK_COPY.micWarnTitle : SYSTEM_CHECK_COPY.checkFailedTitle}
      />
    </div>
  );
};
