import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { toast } from 'sonner';
import { Transcript, TranscriptSegmentData } from '@/types';
import { transcriptService } from '@/services/transcriptService';
import { recordingService } from '@/services/recordingService';
import { useConfig } from '@/contexts/ConfigContext';
import { setContinueSessionActive } from '@/lib/continueRecordingSession';

/** How long to wait for Whisper to drain its queue after stop before giving up. */
const MAX_DRAIN_WAIT_MS = 60_000;
const DRAIN_POLL_INTERVAL_MS = 500;
/** Grace period for transcript-update events that land just after the queue empties. */
const LATE_SEGMENT_GRACE_MS = 2_000;

interface UseContinueRecordingProps {
  meetingId: string;
  meetingTitle: string;
  /** The meeting's existing folder, so the resumed audio lands beside the original. */
  folderPath?: string | null;
  /** Called once the new segments are in the database, to re-read the transcript. */
  onAppended: () => void;
}

interface UseContinueRecordingReturn {
  isRecording: boolean;
  /** Stopped, but still draining the transcription queue and saving. */
  isFinishing: boolean;
  /** Segments captured in this session, for display until the reload lands. */
  liveSegments: TranscriptSegmentData[];
  startContinue: () => Promise<void>;
  stopContinue: () => Promise<void>;
}

/**
 * Resume recording into a meeting that has already been saved.
 *
 * The recording runs in place on the meeting page: new audio is written into
 * the meeting's existing folder, and when it stops the new segments are
 * appended to the same meeting rather than creating a second one.
 *
 * Timestamps are continued rather than restarted -- the offset is applied by
 * `api_append_transcript`, which shifts incoming segments past the last one
 * already stored. Until the reload lands, `liveSegments` shows this session's
 * own clock, which is why they are rendered after the stored ones instead of
 * being sorted in with them.
 */
export function useContinueRecording({
  meetingId,
  meetingTitle,
  folderPath,
  onAppended,
}: UseContinueRecordingProps): UseContinueRecordingReturn {
  const { selectedDevices } = useConfig();

  const [isRecording, setIsRecording] = useState(false);
  const [isFinishing, setIsFinishing] = useState(false);
  const [liveSegments, setLiveSegments] = useState<TranscriptSegmentData[]>([]);

  // Collected off the event stream rather than from React state, so the stop
  // handler sees every segment regardless of render timing.
  const collectedRef = useRef<Transcript[]>([]);
  const unlistenRef = useRef<UnlistenFn | null>(null);

  const detach = useCallback(() => {
    if (unlistenRef.current) {
      unlistenRef.current();
      unlistenRef.current = null;
    }
  }, []);

  // Drop the listener if the page goes away mid-recording. The recording itself
  // is owned by the Rust side and deliberately left running.
  useEffect(() => detach, [detach]);

  const startContinue = useCallback(async () => {
    if (isRecording || isFinishing) return;

    collectedRef.current = [];
    setLiveSegments([]);

    try {
      // Subscribe before starting so nothing transcribed early is missed.
      unlistenRef.current = await transcriptService.onTranscriptUpdate((update) => {
        const segment: Transcript = {
          id: `continued-${update.sequence_id}-${Date.now()}`,
          text: update.text,
          timestamp: update.timestamp,
          sequence_id: update.sequence_id,
          confidence: update.confidence,
          audio_start_time: update.audio_start_time,
          audio_end_time: update.audio_end_time,
          duration: update.duration,
        };
        collectedRef.current = [...collectedRef.current, segment];
        setLiveSegments((prev) => [
          ...prev,
          {
            id: segment.id,
            timestamp: segment.audio_start_time ?? 0,
            endTime: segment.audio_end_time,
            text: segment.text,
            confidence: segment.confidence,
          },
        ]);
      });

      await invoke('start_recording_with_devices_and_meeting', {
        micDeviceName: selectedDevices?.micDevice || null,
        systemDeviceName: selectedDevices?.systemDevice || null,
        meetingName: meetingTitle || 'Continued Meeting',
        existingFolder: folderPath || null,
      });

      setIsRecording(true);
      setContinueSessionActive(true);
      toast.success('Recording resumed', {
        description: 'New audio and transcript will be added to this meeting.',
      });
    } catch (error) {
      detach();
      setContinueSessionActive(false);
      console.error('Failed to resume recording:', error);
      toast.error('Could not resume recording', {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  }, [isRecording, isFinishing, selectedDevices, meetingTitle, folderPath, detach]);

  const stopContinue = useCallback(async () => {
    if (!isRecording) return;

    setIsRecording(false);
    setIsFinishing(true);

    try {
      await recordingService.stopRecording('');

      // Let Whisper finish whatever is still queued; segments keep arriving on
      // the listener while this runs.
      let waited = 0;
      while (waited < MAX_DRAIN_WAIT_MS) {
        try {
          const status = await transcriptService.getTranscriptionStatus();
          if (!status.is_processing && status.chunks_in_queue === 0) break;
        } catch (error) {
          console.error('Error checking transcription status:', error);
          break;
        }
        await new Promise((resolve) => setTimeout(resolve, DRAIN_POLL_INTERVAL_MS));
        waited += DRAIN_POLL_INTERVAL_MS;
      }

      await new Promise((resolve) => setTimeout(resolve, LATE_SEGMENT_GRACE_MS));
      detach();

      const segments = collectedRef.current;
      if (segments.length === 0) {
        toast.info('Nothing new was transcribed', {
          description: 'The meeting was left unchanged.',
        });
        return;
      }

      await invoke('api_append_transcript', {
        meetingId,
        transcripts: segments.map((segment) => ({
          id: segment.id,
          text: segment.text,
          timestamp: segment.timestamp,
          audio_start_time: segment.audio_start_time,
          audio_end_time: segment.audio_end_time,
          duration: segment.duration,
        })),
      });

      collectedRef.current = [];
      setLiveSegments([]);
      onAppended();

      toast.success(`Added ${segments.length} new transcript segments`);
    } catch (error) {
      console.error('Failed to save the resumed recording:', error);
      toast.error('Could not save the new transcript', {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      detach();
      setContinueSessionActive(false);
      setIsFinishing(false);
    }
  }, [isRecording, meetingId, onAppended, detach]);

  return { isRecording, isFinishing, liveSegments, startContinue, stopContinue };
}
