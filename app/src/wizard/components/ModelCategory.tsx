import React, { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Check, ChevronDown } from 'lucide-react';
import { cn } from '@/shared/lib/utils';
import { MODEL_CATEGORY_COPY } from '@/data/welcomeCopy';

interface ModelEntry {
  id: string;
  path: string;
  size: number;
  required: boolean;
}

interface ModelGroup {
  id: string;
  name: string;
  category: string;
  version: string;
  files: ModelEntry[];
}

interface CategoryProps {
    id: string;
    label: string;
    subLabel: string;
    icon: React.ReactNode;
    groups: ModelGroup[];
    selected: boolean;
    required: boolean;
    onToggle: () => void;
    formatSize: (b: number) => string;
    selectedIds: Set<string>;
    onToggleModel: (id: string) => void;
}

export const ModelCategory = ({ 
    label, 
    subLabel, 
    icon, 
    groups, 
    selected, 
    required, 
    onToggle, 
    formatSize,
    selectedIds,
    onToggleModel
}: CategoryProps) => {
    const [isExpanded, setIsExpanded] = useState(false);
    const elementRef = useRef<HTMLDivElement>(null);
    const totalSize = groups.reduce((acc, g) => acc + g.files.reduce((sum, f) => sum + f.size, 0), 0);

    useEffect(() => {
        if (isExpanded && elementRef.current) {
            elementRef.current.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
        }
    }, [isExpanded]);

    return (
        <div 
            ref={elementRef}
            className={cn(
                "w-full rounded-2xl transition-all duration-500 overflow-hidden",
                selected 
                    ? "glass" 
                    : "glass opacity-60 hover:opacity-100"
            )}
        >
            <div 
                className="flex items-center justify-between p-5 cursor-pointer select-none gap-4" 
                onClick={() => setIsExpanded(!isExpanded)}
            >
                <div className="flex items-center gap-4 flex-1 min-w-0">
                    <div className={cn(
                        "w-12 h-12 rounded-xl flex items-center justify-center transition-all duration-500 shrink-0",
                        selected ? "bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.1)]" : "bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))]"
                    )}>
                        {React.cloneElement(icon as React.ReactElement<any>, { className: "w-5 h-5" })}
                    </div>
                    <div className="flex flex-col flex-1 min-w-0">
                        <div className="flex items-center gap-2 mb-1 flex-wrap">
                            <span className="font-display text-[14px] font-black text-[rgb(var(--foreground))] uppercase tracking-[0.15em]">{label}</span>
                            {required ? (
                                <span className="text-[11px] font-black bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] px-2 py-0.5 rounded-full uppercase tracking-tighter border border-[rgb(var(--accent))]/20 shrink-0">{MODEL_CATEGORY_COPY.mandatory}</span>
                            ) : (
                                <span className="text-[11px] font-black bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))] px-2 py-0.5 rounded-full uppercase tracking-tighter border border-[rgba(var(--border),0.08)] shrink-0">{MODEL_CATEGORY_COPY.optional}</span>
                            )}
                        </div>
                        <p className="text-[12px] text-[rgb(var(--foreground-muted))] font-bold uppercase tracking-[0.05em] truncate">
                            {subLabel}
                        </p>
                    </div>
                </div>
                
                <div className="flex items-center gap-6 shrink-0">
                    <div className="flex flex-col items-end gap-1.5">
                        <span className="text-[12px] font-black text-[rgb(var(--accent))] tracking-widest">{formatSize(totalSize)}</span>
                        <div 
                            onClick={(e) => {
                                e.stopPropagation();
                                if (!required) onToggle();
                            }}
                            className={cn(
                                "w-6 h-6 rounded-xl border flex items-center justify-center transition-all duration-300",
                                selected 
                                    ? (required ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))]/40" : "bg-[rgb(var(--accent))] border-transparent shadow-[0_0_20px_rgba(var(--accent),0.5)]")
                                    : "bg-transparent border-[rgba(var(--border),0.15)] hover:border-[rgba(var(--border),0.3)]",
                                required && "cursor-not-allowed"
                            )}
                        >
                            {selected && (
                                <Check className={cn(
                                    "w-4 h-4 stroke-[4]",
                                    required ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))]"
                                )} />
                            )}
                        </div>
                    </div>
                    <div className={cn("transition-transform duration-500", isExpanded && "rotate-180")}>
                        <ChevronDown className="w-5 h-5 text-[rgb(var(--foreground-muted))]" />
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
                        className="border-t border-[rgba(var(--border),0.05)] bg-black/20"
                    >
                        <div className="p-5 space-y-2 max-h-[300px] overflow-y-auto custom-scrollbar">
                            {groups.map((group) => {
                                const isGroupSelected = selectedIds?.has(group.id) ?? false;
                                const groupSize = group.files.reduce((sum, f) => sum + f.size, 0);
                                const handleLineClick = (e: React.MouseEvent) => {
                                    if (required) return;
                                    e.stopPropagation();
                                    onToggleModel?.(group.id);
                                };

                                return (
                                    <div 
                                        key={group.id} 
                                        onClick={handleLineClick}
                                        className={cn(
                                            "flex items-center justify-between text-[12px] font-bold py-2.5 px-4 glass rounded-xl group transition-all border border-transparent hover:border-[rgb(var(--accent))]/20",
                                            !required && "cursor-pointer"
                                        )}
                                    >
                                        <div className="flex items-center gap-3">
                                            <div className={cn(
                                                "w-4 h-4 rounded-md border flex items-center justify-center transition-all duration-300 shrink-0",
                                                isGroupSelected 
                                                    ? (required ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))]/40" : "bg-[rgb(var(--accent))] border-transparent shadow-[0_0_10px_rgba(var(--accent),0.4)]")
                                                    : "bg-transparent border-[rgba(var(--border),0.15)] group-hover:border-[rgba(var(--border),0.3)]"
                                            )}>
                                                {isGroupSelected && (
                                                    <Check className={cn(
                                                        "w-2.5 h-2.5 stroke-[4]",
                                                        required ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))]"
                                                    )} />
                                                )}
                                            </div>
                                            <div className="flex flex-col">
                                                <span className="text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--foreground))] transition-colors truncate max-w-[280px]">
                                                    {group.name}
                                                </span>
                                                <span className="text-[rgb(var(--foreground-muted))]/40 text-[12px] font-mono">
                                                    Version {group.version}
                                                </span>
                                            </div>
                                        </div>
                                        <span className="text-[rgb(var(--foreground-muted))]/40 font-mono text-[12px] group-hover:text-[rgb(var(--accent))]/60 transition-colors self-center">
                                            {formatSize(groupSize)}
                                        </span>
                                    </div>
                                );
                            })}
                        </div>
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
};
