import React, { useState, useEffect } from "react";
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import Image from 'next/image';
import AnalyticsConsentSwitch from "./AnalyticsConsentSwitch";
import { UpdateDialog } from "./UpdateDialog";
import { updateService, UpdateInfo } from '@/services/updateService';
import { Button } from './ui/button';
import { Loader2, CheckCircle2 } from 'lucide-react';
import { toast } from 'sonner';


export function About() {
    const [currentVersion, setCurrentVersion] = useState<string>('0.2.1');
    const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
    const [isChecking, setIsChecking] = useState(false);
    const [showUpdateDialog, setShowUpdateDialog] = useState(false);

    useEffect(() => {
        // Get current version on mount
        getVersion().then(setCurrentVersion).catch(console.error);
    }, []);

    const handleContactClick = async () => {
        try {
            await invoke('open_external_url', { url: 'https://faril.mgh.harvard.edu/' });
        } catch (error) {
            console.error('Failed to open link:', error);
        }
    };

    // Handler for LinkedIn contact (for author name button)
    const handleContactLinkedInClick = async () => {
        try {
            await invoke('open_external_url', { url: 'https://www.linkedin.com/in/nicholasjyee/' });
        } catch (error) {
            console.error('Failed to open LinkedIn link:', error);
        }
    };

    const handleCheckForUpdates = async () => {
        setIsChecking(true);
        try {
            const info = await updateService.checkForUpdates(true);
            setUpdateInfo(info);
            if (info.available) {
                setShowUpdateDialog(true);
            } else {
                toast.success('You are running the latest version');
            }
        } catch (error: any) {
            console.error('Failed to check for updates:', error);
            toast.error('Failed to check for updates: ' + (error.message || 'Unknown error'));
        } finally {
            setIsChecking(false);
        }
    };

    return (
        <div className="p-4 space-y-4 h-[80vh] overflow-y-auto">
            {/* Compact Header */}
            <div className="text-center">
                <div className="mb-1">
                    <Image
                        src="icon_128x128.png"
                        alt="ONOS Logo"
                        width={64}
                        height={64}
                        className="mx-auto my-2"
                    />
                </div>
                {/* <h1 className="text-xl font-bold text-gray-900">ONOS</h1> */}
                <span className="text-sm text-gray-500"> v{currentVersion}</span>
                <p className="text-medium text-gray-600 mt-1">
                    Real-time clinical notes that never leave your machine.
                </p>

                {/* Update button is disabled for now */}
                {/* <div className="mt-3">
                    <Button
                        onClick={handleCheckForUpdates}
                        disabled={isChecking}
                        variant="outline"
                        size="sm"
                        className="text-xs"
                    >
                        {isChecking ? (
                            <>
                                <Loader2 className="h-3 w-3 mr-2 animate-spin" />
                                Checking...
                            </>
                        ) : (
                            <>
                                <CheckCircle2 className="h-3 w-3 mr-2" />
                                Check for Updates
                            </>
                        )}
                    </Button>
                    {updateInfo?.available && (
                        <div className="mt-2 text-xs text-blue-600">
                            Update available: v{updateInfo.version}
                        </div>
                    )}
                </div> */}
            </div>

            {/* Features Grid - Compact */}
            <div className="space-y-3">
                <h2 className="text-base font-semibold text-gray-800">What makes ONOS different</h2>
                <div className="grid grid-cols-2 gap-2">
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">Privacy-first</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">Your data & AI processing workflow can now stay within your premise. No cloud, no leaks.</p>
                    </div>
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">Use Any Model</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">Prefer local open-source model? Great. Want to plug in an external API? Also fine. No lock-in.</p>
                    </div>
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">Cost-Smart</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">Avoid pay-per-minute bills by running models locally (or pay only for the calls you choose).</p>
                    </div>
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">Works everywhere</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">Runs completely offline -- just like your word processor. No internet connection required.</p>
                    </div>
                </div>
            </div>

            {/* Coming Soon - Compact */}
            <div className="bg-blue-50 rounded p-3">
                <p className="text-s text-blue-800">
                    <span className="font-bold">Coming soon:</span> Live translation!
                </p>
            </div>

            {/* CTA Section - Compact */}
            <div className="text-center space-y-2">
                <h3 className="text-medium font-semibold text-gray-800">Looking to contribute?</h3>
                <p className="text-s text-gray-600">
                    Apply to join the Foot & Ankle Research and Innovation Lab (Mass General Brigham and Harvard Medical School).
                </p>
                <button
                    onClick={handleContactClick}
                    className="inline-flex items-center px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded transition-colors duration-200 shadow-sm hover:shadow-md"
                >
                    Apply now
                </button>
            </div>

            {/* Footer - Compact */}
            <div className="pt-2 border-t border-gray-200 text-center">
                <p className="text-xs text-gray-400">
                    Built by{' '}
                    <button
                        onClick={handleContactLinkedInClick}
                        className="underline hover:text-blue-600 transition-colors inline px-1 py-0 m-0 font-semibold text-xs text-gray-500 bg-transparent border-0 outline-none focus:ring-0"
                        style={{ cursor: "pointer", background: "none" }}
                        type="button"
                    >
                        Nicholas J. Yee
                    </button>
                </p>
            </div>

            {/* Logo Section - Four Columns, Doubled Size */}
            <div className="flex justify-center items-center py-4">
                <div
                    className="grid grid-cols-4 gap-8 w-full max-w-xs"
                    style={{
                        maxWidth: "512px" // 4 x 128px, allow extra room, or keep 384px if you prefer more compact
                    }}
                >
                    <div className="flex justify-center items-center">
                        <img
                            src="/uoft.png"
                            alt="University of Toronto Logo"
                            className="h-20 object-contain"
                            style={{ maxWidth: "90px" }}
                        />
                    </div>
                    <div className="flex justify-center items-center">
                        <img
                            src="/MGB.png"
                            alt="Mass General Brigham Logo"
                            className="h-20 object-contain"
                            style={{ maxWidth: "90px" }}
                        />
                    </div>
                    <div className="flex justify-center items-center">
                        <img
                            src="/FARIL.png"
                            alt="FARIL Logo"
                            className="h-20 object-contain"
                            style={{ maxWidth: "90px" }}
                        />
                    </div>
                    <div className="flex justify-center items-center">
                        <img
                            src="/harvard.png"
                            alt="Harvard Logo"
                            className="h-20 object-contain"
                            style={{ maxWidth: "90px" }}
                        />
                    </div>
                </div>
            </div>

            {/* Update Dialog */}
            {/* <UpdateDialog
                open={showUpdateDialog}
                onOpenChange={setShowUpdateDialog}
                updateInfo={updateInfo}
            /> */}
        </div>

    )
}