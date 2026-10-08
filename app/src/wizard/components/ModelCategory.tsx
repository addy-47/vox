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
    icon: React.ReactElement<{ className?: string }>;
    groups: ModelGroup[];
    selected: boolean;
    required: boolean;
    onToggle: () => void;
    formatSize: (b: number) => string;
    selectedIds: Set<string>;
    onToggleModel: (id: string) => void;
}

export const ModelCategory = React.memo(({
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
                "w-full rounded-2xl overflow-hidden",
                selected 
                    ? "glass" 
                    : "glass opacity-60 hover:opacity-100"
            )}
        >
            <div 
                className="flex items-center justify-between p-4 sm:p-5 cursor-pointer select-none gap-3 sm:gap-4" 
                onClick={() => setIsExpanded(!isExpanded)}
                role="button"
                tabIndex={0}
                onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setIsExpanded(!isExpanded); } }}
                aria-label={isExpanded ? `${MODEL_CATEGORY_COPY.collapsePrefix} ${label}` : `${MODEL_CATEGORY_COPY.expandPrefix} ${label}`}
            >
                <div className="flex items-center gap-3 sm:gap-4 flex-1 min-w-0">
                    <div className={cn(
                        "w-10 h-10 sm:w-12 sm:h-12 rounded-xl flex items-center justify-center shrink-0 transition-colors",
                        selected ? "bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.1)]" : "bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))]"
                    )}>
                        {React.cloneElement(icon, { className: "w-5 h-5" })}
                    </div>
                    <div className="flex flex-col flex-1 min-w-0">
                        <div className="flex items-center gap-2 mb-0.5 flex-wrap">
                            <span className="font-display text-[13px] sm:text-[14px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">{label}</span>
                            {required ? (
                                <span className="text-[10px] sm:text-[11px] font-mono font-bold text-[rgb(var(--accent))] uppercase tracking-wider shrink-0">{MODEL_CATEGORY_COPY.mandatory}</span>
                            ) : (
                                <span className="text-[10px] sm:text-[11px] font-mono font-bold text-[rgb(var(--foreground-muted))]/60 uppercase tracking-wider shrink-0">{MODEL_CATEGORY_COPY.optional}</span>
                            )}
                        </div>
                        <p className="text-[12px] text-[rgb(var(--foreground-muted))] font-medium tracking-normal truncate">
                            {subLabel}
                        </p>
                    </div>
                </div>
                
                <div className="flex items-center gap-4 sm:gap-6 shrink-0">
                    <div className="flex flex-col items-end gap-1">
                        <span className="text-[12px] font-bold text-[rgb(var(--accent))] font-mono tracking-normal">{formatSize(totalSize)}</span>
                        <div 
                            onClick={(e) => {
                                e.stopPropagation();
                                if (!required) onToggle();
                            }}
                            role="checkbox"
                            tabIndex={required ? -1 : 0}
                            aria-checked={selected}
                            aria-disabled={required}
                            onKeyDown={(e) => {
                                if (e.key === " " && !required) {
                                    e.preventDefault();
                                    onToggle();
                                }
                            }}
                            className={cn(
                                "w-6 h-6 rounded-xl border flex items-center justify-center transition-colors",
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
                    <div className={cn("transition-transform duration-300", isExpanded && "rotate-180")}>
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
                                        role="checkbox"
                                        tabIndex={required ? -1 : 0}
                                        aria-checked={isGroupSelected}
                                        aria-disabled={required}
                                        onClick={handleLineClick}
                                        onKeyDown={(e) => {
                                            if (e.key === " " && !required) {
                                                e.preventDefault();
                                                onToggleModel?.(group.id);
                                            }
                                        }}
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
                                                <span className="text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--foreground))] transition-colors break-words min-w-0">
                                                    {group.name}
                                                </span>
                                                <span className="text-[rgb(var(--foreground-muted))]/40 text-[12px] font-mono">
                                                    {MODEL_CATEGORY_COPY.versionPrefix} {group.version}
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
});
ModelCategory.displayName = "ModelCategory";
