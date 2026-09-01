import React, { useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { OnboardingContainer } from '../OnboardingContainer';
import { DownloadProgressStep } from './DownloadProgressStep';

export function SetupOverviewStep() {
  const [isMac, setIsMac] = useState(false);
  // Once the user clicks "Let's Go" the downloads start and their progress is
  // shown in place — there is no separate download page to navigate to.
  const [started, setStarted] = useState(false);

  useEffect(() => {
    // Detect platform for totalSteps
    const checkPlatform = async () => {
      try {
        const { platform } = await import('@tauri-apps/plugin-os');
        setIsMac(platform() === 'macos');
      } catch (e) {
        setIsMac(navigator.userAgent.includes('Mac'));
      }
    };
    checkPlatform();
  }, []);

  return (
    <OnboardingContainer
      title={started ? 'Getting things ready' : 'Setup Overview'}
      description={
        started
          ? 'You can start using ONOS after downloading the Transcription Engine.'
          : 'ONOS requires that you download the Transcription & Summarization AI models for the software to work.'
      }
      step={2}
      totalSteps={isMac ? 3 : 2}
    >
      {started ? (
        <DownloadProgressStep embedded />
      ) : (
        <div className="flex flex-col items-center space-y-10">
          <div className="w-full max-w-xs">
            <Button
              onClick={() => setStarted(true)}
              className="w-full h-11 bg-gray-900 hover:bg-gray-800 text-white"
            >
              Let's Go
            </Button>
          </div>
        </div>
      )}
    </OnboardingContainer>
  );
}
