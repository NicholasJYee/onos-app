"use client";

import { Transcript, TranscriptSegmentData } from '@/types';
import { TranscriptView } from '@/components/TranscriptView';
import { VirtualizedTranscriptView } from '@/components/VirtualizedTranscriptView';
import { TranscriptButtonGroup } from './TranscriptButtonGroup';
import { Mic, Square, Loader2 } from 'lucide-react';
import { useMemo } from 'react';

interface TranscriptPanelProps {
  transcripts: Transcript[];
  customPrompt: string;
  onPromptChange: (value: string) => void;
  onCopyTranscript: () => void;
  onOpenMeetingFolder: () => Promise<void>;
  isRecording: boolean;
  disableAutoScroll?: boolean;

  // Optional pagination props (when using virtualization)
  usePagination?: boolean;
  segments?: TranscriptSegmentData[];
  hasMore?: boolean;
  isLoadingMore?: boolean;
  totalCount?: number;
  loadedCount?: number;
  onLoadMore?: () => void;
  /**
   * Phone-width only: whether this panel is the selected tab. Desktop always
   * shows both panels side by side, so this has no effect at `md` and above.
   */
  mobileVisible?: boolean;
  /** Resume recording into this meeting. Hidden when not provided. */
  onContinueRecording?: () => void;
  onStopContinue?: () => void;
  isContinuing?: boolean;
  isFinishingContinue?: boolean;
}

export function TranscriptPanel({
  transcripts,
  customPrompt,
  onPromptChange,
  onCopyTranscript,
  onOpenMeetingFolder,
  isRecording,
  disableAutoScroll = false,
  mobileVisible = false,
  onContinueRecording,
  onStopContinue,
  isContinuing = false,
  isFinishingContinue = false,
  usePagination = false,
  segments,
  hasMore,
  isLoadingMore,
  totalCount,
  loadedCount,
  onLoadMore,
}: TranscriptPanelProps) {
  // Convert transcripts to segments if pagination is not used but we want virtualization
  const convertedSegments = useMemo(() => {
    if (usePagination && segments) {
      return segments;
    }
    // Convert transcripts to segments for virtualization
    return transcripts.map(t => ({
      id: t.id,
      timestamp: t.audio_start_time ?? 0,
      endTime: t.audio_end_time,
      text: t.text,
      confidence: t.confidence,
    }));
  }, [transcripts, usePagination, segments]);

  return (
    <div className={`${mobileVisible ? 'flex w-full' : 'hidden'} md:flex md:w-1/4 lg:w-1/3 min-w-0 border-r border-gray-200 bg-white flex-col relative shrink-0`}>
      {/* Title area */}
      <div className="p-4 border-b border-gray-200">
        <TranscriptButtonGroup
          transcriptCount={usePagination ? (totalCount ?? convertedSegments.length) : (transcripts?.length || 0)}
          onCopyTranscript={onCopyTranscript}
          onOpenMeetingFolder={onOpenMeetingFolder}
        />

        {/* Resume this meeting: new audio goes to the same folder and the new
            transcript is appended below what is already here. */}
        {onContinueRecording && (
          <div className="mt-3">
            {isFinishingContinue ? (
              <button
                type="button"
                disabled
                className="w-full flex items-center justify-center gap-2 px-4 py-2 rounded-md text-sm font-medium bg-gray-100 text-gray-500 cursor-not-allowed"
              >
                <Loader2 className="w-4 h-4 animate-spin" />
                Saving transcript...
              </button>
            ) : isContinuing ? (
              <button
                type="button"
                onClick={onStopContinue}
                className="w-full flex items-center justify-center gap-2 px-4 py-2 rounded-md text-sm font-medium bg-red-600 text-white hover:bg-red-700 transition-colors"
              >
                <Square className="w-4 h-4" fill="currentColor" />
                Stop Recording
                <span className="ml-1 w-2 h-2 rounded-full bg-white animate-pulse" />
              </button>
            ) : (
              <button
                type="button"
                onClick={onContinueRecording}
                className="w-full flex items-center justify-center gap-2 px-4 py-2 rounded-md text-sm font-medium border border-gray-300 text-gray-700 hover:bg-gray-50 transition-colors"
              >
                <Mic className="w-4 h-4" />
                Continue Recording
              </button>
            )}
          </div>
        )}
      </div>

      {/* Transcript content - use virtualized view for better performance */}
      <div className="flex-1 overflow-hidden pb-4">
        <VirtualizedTranscriptView
          segments={convertedSegments}
          isRecording={isRecording}
          isPaused={false}
          isProcessing={false}
          isStopping={false}
          enableStreaming={false}
          showConfidence={true}
          disableAutoScroll={disableAutoScroll}
          hasMore={hasMore}
          isLoadingMore={isLoadingMore}
          totalCount={totalCount}
          loadedCount={loadedCount}
          onLoadMore={onLoadMore}
        />
      </div>

      {/* Custom prompt input at bottom of transcript section */}
      {!isRecording && convertedSegments.length > 0 && (
        <div className="p-1 border-t border-gray-200">
          <textarea
            placeholder="Add context for the clinic note. For example patient name, medical details, note corrections, etc."
            className="w-full px-3 py-2 border border-gray-200 rounded-md text-sm focus:outline-none focus:ring-1 focus:ring-blue-500 focus:border-blue-500 bg-white shadow-sm min-h-[80px] resize-y"
            value={customPrompt}
            onChange={(e) => onPromptChange(e.target.value)}
          />
        </div>
      )}
    </div>
  );
}
