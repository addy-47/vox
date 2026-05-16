import React, { useState, useEffect, useMemo, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Check, Database, ChevronDown, FileCode } from 'lucide-react';
import { cn } from '@/shared/lib/utils';

interface ModelEntry {
  id: string;
  path: string;
  size: number;
  required: boolean;
}

interface FolderNode {
    name: string;
    files: ModelEntry[];
    folders: Record<string, FolderNode>;
}

interface CategoryProps {
    id: string;
    label: string;
    subLabel: string;
    icon: React.ReactNode;
    models: ModelEntry[];
    selected: boolean;
    required: boolean;
    onToggle: () => void;
    formatSize: (b: number) => string;
}

export const ModelCategory = ({ 
    label, 
    subLabel, 
    icon, 
    models, 
    selected, 
    required, 
    onToggle, 
    formatSize 
}: CategoryProps) => {
    const [isExpanded, setIsExpanded] = useState(false);
    const elementRef = useRef<HTMLDivElement>(null);
    const totalSize = models.reduce((acc, m) => acc + m.size, 0);

    useEffect(() => {
        if (isExpanded && elementRef.current) {
            elementRef.current.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
        }
    }, [isExpanded]);

    // Build tree structure for this category
    const tree = useMemo(() => {
        const root: FolderNode = { name: 'root', files: [], folders: {} };
        models.forEach(m => {
            const parts = m.path.split('/');
            const relevantParts = parts.slice(1);
            let current = root;
            
            relevantParts.forEach((part, i) => {
                if (i === relevantParts.length - 1) {
                    current.files.push(m);
                } else {
                    if (!current.folders[part]) {
                        current.folders[part] = { name: part, files: [], folders: {} };
                    }
                    current = current.folders[part];
                }
            });
        });
        return root;
    }, [models]);

    const renderFolder = (node: FolderNode) => {
        return (
            <div key={node.name} className="relative">
                {node.name !== 'root' && (
                    <div className="flex items-center gap-2 py-2 px-3 text-[11px] font-black text-white/90 uppercase tracking-[0.2em] bg-white/5 rounded-xl mb-2 border border-white/10 shadow-sm">
                        <Database className="w-3.5 h-3.5 text-[#00dbe9]" />
                        {node.name}
                    </div>
                )}
                <div className={cn("space-y-1 relative", node.name !== 'root' && "ml-4 pl-4 border-l border-white/10")}>
                    {Object.values(node.folders).map((f) => renderFolder(f))}
                    {node.files.map(m => (
                        <div key={m.id} className="flex items-center justify-between text-[11px] font-bold py-2 px-3 hover:bg-[#00dbe9]/5 rounded-xl group transition-all border border-transparent hover:border-[#00dbe9]/20">
                            <div className="flex items-center gap-3">
                                <div className="relative">
                                    <div className="absolute inset-0 bg-[#00dbe9] blur-md opacity-0 group-hover:opacity-20 transition-opacity" />
                                    <FileCode className="w-4 h-4 text-white/40 group-hover:text-[#00dbe9] transition-colors relative z-10" />
                                </div>
                                <span className="text-white/60 group-hover:text-white transition-colors truncate max-w-[280px]">{m.path.split('/').pop()}</span>
                            </div>
                            <span className="text-white/40 font-mono text-[10px] group-hover:text-[#00dbe9]/60 transition-colors">{formatSize(m.size)}</span>
                        </div>
                    ))}
                </div>
            </div>
        );
    };

    return (
        <div 
            ref={elementRef}
            className={cn(
                "w-full rounded-2xl border transition-all duration-500 overflow-hidden",
                selected 
                    ? "bg-white/[0.04] border-white/20 shadow-[0_0_30px_rgba(0,0,0,0.3)]" 
                    : "bg-white/[0.01] border-white/5 opacity-50 hover:opacity-100"
            )}
        >
            <div 
                className="flex items-center justify-between p-5 cursor-pointer select-none" 
                onClick={() => setIsExpanded(!isExpanded)}
            >
                <div className="flex items-center gap-5">
                    <div className={cn(
                        "w-12 h-12 rounded-xl flex items-center justify-center transition-all duration-500",
                        selected ? "bg-[#00dbe9]/10 text-[#00dbe9] shadow-[0_0_20px_rgba(0,219,233,0.1)]" : "bg-white/5 text-white/40"
                    )}>
                        {React.cloneElement(icon as React.ReactElement<any>, { className: "w-5 h-5" })}
                    </div>
                    <div className="flex flex-col">
                        <div className="flex items-center gap-3 mb-1">
                            <span className="text-[13px] font-black text-white uppercase tracking-[0.2em]">{label}</span>
                            {required ? (
                                <span className="text-[9px] font-black bg-[#00dbe9]/20 text-[#00dbe9] px-2 py-0.5 rounded-full uppercase tracking-tighter border border-[#00dbe9]/20">Mandatory</span>
                            ) : (
                                <span className="text-[9px] font-black bg-white/5 text-white/40 px-2 py-0.5 rounded-full uppercase tracking-tighter border border-white/10">Optional Layer</span>
                            )}
                        </div>
                        <p className="text-[10px] text-white/60 font-bold uppercase tracking-[0.1em]">
                            {subLabel}
                        </p>
                    </div>
                </div>
                
                <div className="flex items-center gap-8">
                    <div className="flex flex-col items-end gap-1.5">
                        <span className="text-[11px] font-black text-[#00dbe9] tracking-widest shadow-[#00dbe9]/20 drop-shadow-sm">{formatSize(totalSize)}</span>
                        <div 
                            onClick={(e) => {
                                e.stopPropagation();
                                if (!required) onToggle();
                            }}
                            className={cn(
                                "w-6 h-6 rounded-xl border flex items-center justify-center transition-all duration-300",
                                selected 
                                    ? (required ? "bg-[#00dbe9]/10 border-[#00dbe9]/40 shadow-[0_0_15px_rgba(0,219,233,0.1)]" : "bg-[#00dbe9] border-transparent shadow-[0_0_20px_rgba(0,219,233,0.5)]")
                                    : "bg-transparent border-white/20 hover:border-white/40",
                                required && "cursor-not-allowed"
                            )}
                        >
                            {selected && (
                                <Check className={cn(
                                    "w-4 h-4 stroke-[4]",
                                    required ? "text-[#00dbe9]" : "text-black"
                                )} />
                            )}
                        </div>

                    </div>
                    <div className={cn("transition-transform duration-500", isExpanded && "rotate-180")}>
                        <ChevronDown className="w-5 h-5 text-white/40" />
                    </div>
                </div>
            </div>

            <AnimatePresence>
                {isExpanded && (
                    <motion.div 
                        initial={{ height: 0, opacity: 0 }}
                        animate={{ height: 'auto', opacity: 1 }}
                        exit={{ height: 0, opacity: 0 }}
                        transition={{ duration: 0.3, ease: 'circOut' }}
                        className="border-t border-white/5 bg-black/40"
                    >
                        <div className="p-5 space-y-4 max-h-[300px] overflow-y-auto custom-scrollbar">
                            {renderFolder(tree)}
                        </div>
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
};
