

export class VideoDecoder {
    constructor(canvas) {
        if (!canvas) {
            console.error(' VideoDecoder: Canvas element is null or undefined');
            this.canvas = null;
            this.ctx = null;
            this.fallbackToImageDecoding = true;
        } else if (canvas instanceof HTMLCanvasElement) {
            this.canvas = canvas;
            this.ctx = canvas.getContext('2d');
            if (!this.ctx) {
                console.error(' VideoDecoder: Failed to get 2D context from canvas');
                this.fallbackToImageDecoding = true;
            }
        } else if (canvas && canvas.canvas && canvas.canvas instanceof HTMLCanvasElement) {
            this.ctx = canvas;
            this.canvas = canvas.canvas;
            if (!this.ctx) {
                this.ctx = this.canvas.getContext('2d');
            }
        } else {
            console.error(' VideoDecoder: Invalid canvas element provided');
            this.canvas = null;
            this.ctx = null;
            this.fallbackToImageDecoding = true;
        }

        this.decoder = null;
        this.decoderState = 'unconfigured';
        this.waitingForKeyframe = false;
        this.isDecoding = false;
        this.frameQueue = [];
        this.supportedCodecs = [];
        this.fallbackToImageDecoding = this.fallbackToImageDecoding || false;
        this.webCodecsSupported = false;
        this.currentCodec = null;
        this.frameCount = 0;
        this.errorCount = 0;
        this.maxErrors = 10;

        this.initializeDecoder();
    }

    async initializeDecoder() {
        try {

            if ('VideoDecoder' in window) {
                console.log(' WebCodecs API supported');
                this.supportedCodecs = ['vp09.00.10.08', 'vp8', 'avc1.42E01E', 'hev1.1.6.L93.B0'];
                console.log(' Assuming common codecs are supported, will test during decode');

                this.webCodecsSupported = true;
            } else {
                console.warn(' WebCodecs API not supported, falling back to image decoding');
                this.fallbackToImageDecoding = true;
            }
        } catch (error) {
            console.error(' Error initializing video decoder:', error);
            this.fallbackToImageDecoding = true;
        }
    }

    createDecoder(codecType) {
        return new Promise(async (resolve) => {
            try {

                if (this.decoder && this.currentCodec === codecType && this.decoderState === 'configured') {
                    resolve(true);
                    return;
                }

                if (this.decoder && (this.currentCodec !== codecType || this.decoderState === 'closed')) {
                    this.closeDecoder();
                }

                const codecMap = {
                    1: 'vp8',
                    2: 'vp09.00.10.08',
                    3: 'avc1.42E01E',
                    4: 'hev1.1.6.L93.B0'
                };

                const codec = codecMap[codecType];
                if (!codec) {
                    console.error(` Unknown codec type: ${codecType}`);
                    resolve(false);
                    return;
                }

                console.log(` Creating VideoDecoder for codec: ${codec}`);

                if (typeof window.VideoDecoder === 'undefined') {
                    console.error(' VideoDecoder is not available');
                    this.fallbackToImageDecoding = true;
                    resolve(false);
                    return;
                }
                this.decoder = new window.VideoDecoder({
                    output: (frame) => {
                        try {

                            this.renderFrame(frame);
                            frame.close();
                        } catch (error) {
                            console.error(' Error rendering frame:', error);
                            frame.close();
                        }
                    },
                    error: (error) => {
                        console.error(' VideoDecoder error:', error);
                        this.decoderState = 'closed';
                        this.handleDecoderError(error);
                    }
                });

                if (!this.decoder || typeof this.decoder.configure !== 'function') {
                    console.error(' VideoDecoder creation failed - configure method not available');
                    this.fallbackToImageDecoding = true;
                    resolve(false);
                    return;
                }

                const config = {
                    codec: codec,
                    hardwareAcceleration: 'prefer-hardware'
                };


                if (codec.startsWith('vp09')) {

                    if (!codec.match(/^vp09\.00\.10\.(08|0a|0b)$/)) {
                        console.warn(` Using standard VP9 profile 0 for codec: ${codec}`);
                    }
                }

                if (typeof window.VideoDecoder.isConfigSupported === 'function') {
                    try {
                        const supportResult = await window.VideoDecoder.isConfigSupported(config);
                        if (!supportResult.supported) {
                            console.warn(` Codec ${codec} not supported, trying fallback configuration`);

                            config.hardwareAcceleration = 'prefer-software';
                            const fallbackResult = await window.VideoDecoder.isConfigSupported(config);
                            if (!fallbackResult.supported) {
                                console.error(` Codec ${codec} not supported even with software acceleration`);
                                this.fallbackToImageDecoding = true;
                                resolve(false);
                                return;
                            }
                        }
                    } catch (error) {
                        console.warn(' isConfigSupported check failed, proceeding with configuration:', error);
                    }
                }

                try {
                    this.decoder.configure(config);
                    this.currentCodec = codecType;
                    this.decoderState = 'configured';
                    this.waitingForKeyframe = true;
                    console.log(` VideoDecoder configured for ${codec}, waiting for keyframe`);
                    resolve(true);
                } catch (configError) {
                    console.error(` Failed to configure decoder for ${codec}:`, configError);
                    this.decoderState = 'closed';
                    this.fallbackToImageDecoding = true;
                    resolve(false);
                }

            } catch (error) {
                console.error(' Error creating VideoDecoder:', error);
                this.fallbackToImageDecoding = true;
                resolve(false);
            }
        });
    }

    async decodeVideoFrame(codecType, frameData, isKeyframe = null) {
        try {
            this.frameCount++;

            if (!this.webCodecsSupported || this.fallbackToImageDecoding) {
                return this.decodeAsImage(frameData);
            }


            if (!this.decoder || this.currentCodec !== codecType || this.decoderState !== 'configured') {
                const success = await this.createDecoder(codecType);
                if (!success) {
                    return this.decodeAsImage(frameData);
                }
            }

            if (!this.decoder || this.decoderState !== 'configured') {
                console.warn(' Decoder not in valid state for decoding, falling back to image');
                return this.decodeAsImage(frameData);
            }


            let isKeyFrame;
            if (isKeyframe !== null) {
                isKeyFrame = isKeyframe;
                // console.log(` Using packet keyframe info: ${isKeyFrame} (decoder waiting: ${this.waitingForKeyframe})`);
            } else {

                isKeyFrame = this.detectKeyFrame(frameData, codecType);
                console.log(`🔍 Detected keyframe: ${isKeyFrame} (decoder waiting: ${this.waitingForKeyframe})`);
            }

            if (this.waitingForKeyframe) {
                if (!isKeyFrame) {

                    console.log('⏭️ Skipping non-keyframe while waiting for keyframe after decoder configure');
                    return;
                } else {

                    this.waitingForKeyframe = false;
                    console.log(' Received keyframe after decoder configure - proceeding with decode');
                    if (codecType === 2) {
                        this.validateVp9Keyframe(frameData);
                    }
                }
            }

            if (this.decoderState !== 'configured') {
                console.warn(' Decoder not in configured state, skipping decode');
                return;
            }

            const chunk = new window.EncodedVideoChunk({
                type: isKeyFrame ? 'key' : 'delta',
                timestamp: performance.now() * 1000,
                data: frameData
            });

            if (!this.decoder || this.decoderState !== 'configured') {
                console.warn(' Decoder not in configured state, skipping decode');
                return;
            }

            try {
                this.decoder.decode(chunk);

                this.errorCount = 0;

            } catch (decodeError) {

                console.error(' Decoder.decode() failed:', decodeError);
                if (decodeError.name === 'DataError' && decodeError.message.includes('key frame is required')) {

                    console.log(' Keyframe error detected, will reset decoder on next frame');
                    this.waitingForKeyframe = true;
                }
                throw decodeError;
            }

        } catch (error) {
            console.error(' Error decoding video frame:', error);
            this.handleDecoderError(error);

            if (codecType === 0) {
                return this.decodeAsImage(frameData);
            } else {
                console.warn(` Skipping image fallback for codec ${codecType} (not JPEG)`);
            }
        }

    }

    detectKeyFrame(frameData, codecType) {
        try {

            if (codecType === 2) {
                const view = new Uint8Array(frameData);
                if (view.length > 1) {

                    const byte0 = view[0];


                    const frameMarker = (byte0 >> 6) & 0x03;
                    if (frameMarker !== 0x02) {
                        console.warn(` Invalid VP9 frame marker: ${frameMarker}, assuming keyframe`);
                        return true;
                    }

                    const frameType = byte0 & 0x01;

                    const errorResilientMode = (byte0 >> 2) & 0x01;

                    const isKeyFrame = frameType === 0;


                    if (isKeyFrame && view.length > 9) {

                        return true;
                    }


                    return isKeyFrame;
                }
            }

            if (codecType === 1) {
                const view = new Uint8Array(frameData);
                if (view.length > 3) {
                    const frameTag = (view[0] << 16) | (view[1] << 8) | view[2];
                    return (frameTag & 0x00FFFFFF) === 0x009D012A;
                }
            }

            if (codecType === 3 || codecType === 4) {

                return this.frameCount < 5 || this.frameCount % 30 === 0;
            }

            return true;
        } catch (error) {
            console.warn(' Error detecting frame type, assuming keyframe:', error);
            return true;
        }
    }

    validateVp9Keyframe(frameData) {
        try {
            const view = new Uint8Array(frameData);
            if (view.length > 1) {

                const frameMarker = view[0] & 0xE0;
                const profile = (view[0] >> 4) & 0x07;
                const showFrame = view[0] & 0x01;

                if (frameMarker === 0x80 && showFrame === 1) {
                    console.log(` VP9 keyframe validation passed: profile=${profile}`);
                    return true;
                } else {
                    console.warn(` VP9 frame validation failed: marker=0x${frameMarker.toString(16)}, showFrame=${showFrame}`);
                    return false;
                }
            }
            return false;
        } catch (error) {
            console.error(' Error validating VP9 keyframe:', error);
            return false;
        }
    }

    handleDecoderError(error) {
        this.errorCount++;
        console.error(` Decoder error #${this.errorCount}:`, error);

        this.decoderState = 'closed';

        if (this.errorCount >= this.maxErrors) {
            console.warn(` Too many decoder errors (${this.errorCount}), permanently falling back to image decoding`);
            this.fallbackToImageDecoding = true;
            this.cleanup();
        } else {
            console.log(` Attempting to recover from decoder error (${this.errorCount}/${this.maxErrors})`);

            this.resetDecoder();
        }
    }

    resetDecoder() {
        try {

            if (this.decoder && this.decoderState !== 'closed') {
                try {

                    if (typeof this.decoder.flush === 'function') {
                        this.decoder.flush();
                    }
                    this.decoder.close();
                } catch (closeError) {
                    console.warn(' Error closing decoder during reset:', closeError);
                }
            }
            this.decoder = null;
            this.currentCodec = null;
            this.decoderState = 'unconfigured';
            this.waitingForKeyframe = false;
            console.log(' Decoder reset completed');
        } catch (error) {
            console.error(' Error resetting decoder:', error);
            this.decoder = null;
            this.decoderState = 'unconfigured';
            this.waitingForKeyframe = false;
        }
    }

    closeDecoder() {
        try {
            if (this.decoder && this.decoderState !== 'closed') {
                try {
                    this.decoder.close();
                } catch (closeError) {
                    console.warn(' Error closing decoder:', closeError);
                }
            }
            this.decoder = null;
            this.currentCodec = null;
            this.decoderState = 'closed';
            this.waitingForKeyframe = false;
        } catch (error) {
            console.error(' Error closing decoder:', error);
            this.decoder = null;
            this.decoderState = 'closed';
            this.waitingForKeyframe = false;
        }
    }

    cleanup() {
        try {
            this.closeDecoder();
            console.log('🧹 VideoDecoder cleanup completed');
        } catch (error) {
            console.error(' Error during cleanup:', error);
        }
    }

    renderFrame(frame) {
        try {
            const canvas = this.canvas || document.getElementById('Desk') || document.getElementById('DeskArea');
            if (!canvas) {
                console.error(' Canvas element not found');
                return;
            }

            const ctx = this.ctx || canvas.getContext('2d');
            if (!ctx) {
                console.error(' Could not get canvas context');
                return;
            }

            if (canvas.width !== frame.codedWidth || canvas.height !== frame.codedHeight) {
                canvas.width = frame.codedWidth;
                canvas.height = frame.codedHeight;
                console.log(` Canvas resized to ${frame.codedWidth}x${frame.codedHeight}`);
            }

            ctx.drawImage(frame, 0, 0);

        } catch (error) {
            console.error(' Error rendering frame to canvas:', error);
        }
    }

    decodeAsImage(frameData) {
        try {
            let canvas = this.canvas;
            let ctx = this.ctx;

            if (!canvas || !ctx) {
                canvas = document.getElementById('Desk') || document.getElementById('DeskArea');
                if (!canvas) {
                    console.error(' No canvas element available for image decoding');
                    return false;
                }
                ctx = canvas.getContext('2d');
                if (!ctx) {
                    console.error(' Could not get canvas context for image decoding');
                    return false;
                }

                this.canvas = canvas;
                this.ctx = ctx;
            }

            if (!this.isLikelyImageBuffer(frameData)) {
                console.warn(' Frame data does not appear to be an image; skipping image fallback');
                return false;
            }

            const imageFormats = [
                { type: 'image/jpeg', name: 'JPEG' },
                { type: 'image/png', name: 'PNG' },
                { type: 'image/webp', name: 'WebP' },
                { type: 'image/bmp', name: 'BMP' }
            ];

            let formatIndex = 0;

            const tryNextFormat = () => {
                if (formatIndex >= imageFormats.length) {
                    console.error(' Failed to decode frame with any image format');
                    return false;
                }

                const format = imageFormats[formatIndex];
                const blob = new Blob([frameData], { type: format.type });
                const url = URL.createObjectURL(blob);

                const img = new Image();
                img.onload = () => {
                    try {

                        if (canvas.width !== img.width || canvas.height !== img.height) {
                            canvas.width = img.width;
                            canvas.height = img.height;
                        }

                        ctx.clearRect(0, 0, canvas.width, canvas.height);
                        ctx.drawImage(img, 0, 0);
                        URL.revokeObjectURL(url);
                        console.log(`Frame decoded as ${format.name} image (${img.width}x${img.height})`);
                    } catch (drawError) {
                        console.error(' Error drawing image to canvas:', drawError);
                        URL.revokeObjectURL(url);
                    }
                };

                img.onerror = () => {
                    URL.revokeObjectURL(url);
                    formatIndex++;
                    if (formatIndex < imageFormats.length) {
                        tryNextFormat();
                    } else {
                        console.error(' Failed to decode frame as any image format');
                    }
                };

                img.src = url;
            };

            tryNextFormat();
            return true;

        } catch (error) {
            console.error(' Error in image fallback:', error);
            return false;
        }
    }

    isLikelyImageBuffer(buffer) {
        try {
            const bytes = buffer instanceof Uint8Array ? buffer : new Uint8Array(buffer);
            if (bytes.length < 12) return false;
            if (bytes[0] === 0xFF && bytes[1] === 0xD8 && bytes[2] === 0xFF) return true;
            const pngSig = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
            let isPng = true;
            for (let i = 0; i < pngSig.length; i++) {
                if (bytes[i] !== pngSig[i]) { isPng = false; break; }
            }
            if (isPng) return true;

            const riff = String.fromCharCode(bytes[0], bytes[1], bytes[2], bytes[3]);
            const webp = String.fromCharCode(bytes[8], bytes[9], bytes[10], bytes[11]);
            if (riff === 'RIFF' && webp === 'WEBP') return true;
            if (bytes[0] === 0x42 && bytes[1] === 0x4D) return true;
            return false;
        } catch (_) {
            return false;
        }
    }

    close() {
        this.closeDecoder();
    }
}