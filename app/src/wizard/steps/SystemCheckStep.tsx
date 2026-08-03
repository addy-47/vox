import React, { useEffect, useState } from 'react';
import { HardDrive, Cpu, Database, Mic } from 'lucide-react';
import { motion } from 'framer-motion';
import { getRuntimeReport, type RuntimeReport } from '@/services/modelService';

// --- Modular Components ---
import { WizardHeader } from '../components/WizardHeader';
import { WizardFooter } from '../components/WizardFooter';
import { StatusCard } from '../components/StatusCard';

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

  const allOk = report && 
    report.write_access && 
    report.disk_space_ok && 
    report.mic_access;

  return (
    <div className="flex flex-col h-full relative">
      <WizardHeader 
        step="Step 2.0 • Infrastructure"
        title="Environment Scan"
        description="Analyzing system environment for local AI execution. We ensure your hardware meets the requirements for a seamless experience."
      />

      <div className="flex-1 overflow-y-auto pr-2 custom-scrollbar">
        <div className="grid grid-cols-2 gap-4">
            <motion.div
                key="storage"
                initial={{ opacity: 0, y: 5 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.1 }}
            >
                <StatusCard 
                    icon={<HardDrive className="w-4 h-4" />}
                    label="STORAGE"
                    value={report ? `${report.available_space_gb.toFixed(1)} GB Available` : "Scanning..."}
                    subValue={report ? `System Total: ${report.total_space_gb.toFixed(1)} GB | Required: ${report.required_space_gb.toFixed(1)} GB` : "Calculating requirements..."}
                    ok={report?.disk_space_ok}
                    loading={isLoading}
                />
            </motion.div>
            <motion.div
                key="audio"
                initial={{ opacity: 0, y: 5 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.15 }}
            >
                <StatusCard 
                    icon={<Mic className="w-4 h-4" />}
                    label="AUDIO"
                    value={report ? (report.mic_access ? "CONNECTED" : "NOT FOUND") : "Scanning..."}
                    subValue={report ? (report.mic_access ? "Input device detected" : "Check permissions") : "Scanning hardware..."}
                    ok={report?.mic_access}
                    loading={isLoading}
                />
            </motion.div>
            <motion.div
                key="permissions"
                initial={{ opacity: 0, y: 5 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.2 }}
            >
                <StatusCard 
                    icon={<Database className="w-4 h-4" />}
                    label="PERMISSIONS"
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
                    label="HARDWARE"
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
        nextLabel="Proceed to Model Sync"
        isNextDisabled={!allOk || isLoading}
        showBack={true}
        error={externalError || (!allOk && !isLoading ? "Constraints Detected" : undefined)}
        errorLabel="Infrastructure Error"
      />
    </div>
  );
};


