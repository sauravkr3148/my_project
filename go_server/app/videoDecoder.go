package relay

import (
	"encoding/binary"
	"fmt"
	"log"
)

type VideoCodec int

const (
	CodecVP8 VideoCodec = iota
	CodecVP9
	CodecH264
	CodecH265
	CodecAV1
)

type DecodedFrame struct {
	Width      int
	Height     int
	Data       []byte
	Timestamp  int64
	IsKeyframe bool
	Format     string
}

type VideoDecoder struct {
	codec       VideoCodec
	width       int
	height      int
	frameCount  uint64
	initialized bool
}

func NewVideoDecoder(codec VideoCodec, width, height int) *VideoDecoder {
	return &VideoDecoder{
		codec:       codec,
		width:       width,
		height:      height,
		frameCount:  0,
		initialized: true,
	}
}

func (d *VideoDecoder) DecodeFrame(frameData []byte) (*DecodedFrame, error) {
	if !d.initialized {
		return nil, fmt.Errorf("decoder not initialized")
	}

	if len(frameData) == 0 {
		return nil, fmt.Errorf("empty frame data")
	}

	d.frameCount++

	isKeyframe := d.isKeyframe(frameData)

	decoded := &DecodedFrame{
		Width:      d.width,
		Height:     d.height,
		Data:       frameData,
		Timestamp:  0,
		IsKeyframe: isKeyframe,
		Format:     d.getCodecName(),
	}

	if d.frameCount%100 == 0 {
		log.Printf("Decoded %d frames (codec: %s, keyframe: %v, size: %d bytes)",
			d.frameCount, d.getCodecName(), isKeyframe, len(frameData))
	}

	return decoded, nil
}

func (d *VideoDecoder) isKeyframe(data []byte) bool {
	if len(data) < 4 {
		return false
	}

	switch d.codec {
	case CodecVP8:

		return (data[0] & 0x01) == 0

	case CodecVP9:

		return (data[0] & 0x01) == 0

	case CodecH264:

		for i := 0; i < len(data)-4; i++ {
			if data[i] == 0x00 && data[i+1] == 0x00 {
				if data[i+2] == 0x00 && data[i+3] == 0x01 {
					if i+4 < len(data) {
						nalType := data[i+4] & 0x1F
						if nalType == 5 {
							return true
						}
					}
				} else if data[i+2] == 0x01 {
					if i+3 < len(data) {
						nalType := data[i+3] & 0x1F
						if nalType == 5 {
							return true
						}
					}
				}
			}
		}
		return false

	case CodecH265:
		for i := 0; i < len(data)-4; i++ {
			if data[i] == 0x00 && data[i+1] == 0x00 {
				if data[i+2] == 0x00 && data[i+3] == 0x01 {
					if i+4 < len(data) {
						nalType := (data[i+4] >> 1) & 0x3F
						if nalType >= 16 && nalType <= 23 {
							return true
						}
					}
				}
			}
		}
		return false

	default:
		return false
	}
}

func (d *VideoDecoder) getCodecName() string {
	switch d.codec {
	case CodecVP8:
		return "VP8"
	case CodecVP9:
		return "VP9"
	case CodecH264:
		return "H264"
	case CodecH265:
		return "H265"
	case CodecAV1:
		return "AV1"
	default:
		return "Unknown"
	}
}

func (d *VideoDecoder) Reset() {
	d.frameCount = 0
	log.Printf("Decoder reset (codec: %s)", d.getCodecName())
}

func (d *VideoDecoder) GetFrameCount() uint64 {
	return d.frameCount
}

func ParseVideoPacket(data []byte) (codec VideoCodec, frameData []byte, err error) {
	if len(data) < 2 {
		return 0, nil, fmt.Errorf("packet too small")
	}

	codecID := data[1]

	switch codecID {
	case 0:
		codec = CodecVP8
	case 1:
		codec = CodecVP9
	case 2:
		codec = CodecH264
	case 3:
		codec = CodecH265
	case 4:
		codec = CodecAV1
	default:
		return 0, nil, fmt.Errorf("unknown codec ID: %d", codecID)
	}

	frameData = data[2:]

	return codec, frameData, nil
}

func CreateVideoFramePacket(codecID byte, frameData []byte) []byte {
	packet := make([]byte, 2+len(frameData))
	packet[0] = 7
	packet[1] = codecID
	copy(packet[2:], frameData)
	return packet
}

type FrameStatistics struct {
	TotalFrames      uint64
	KeyFrames        uint64
	DroppedFrames    uint64
	AverageFrameSize uint64
	TotalBytes       uint64
}

type VideoDecoderWithStats struct {
	*VideoDecoder
	stats FrameStatistics
}

func NewVideoDecoderWithStats(codec VideoCodec, width, height int) *VideoDecoderWithStats {
	return &VideoDecoderWithStats{
		VideoDecoder: NewVideoDecoder(codec, width, height),
		stats:        FrameStatistics{},
	}
}

func (d *VideoDecoderWithStats) DecodeFrameWithStats(frameData []byte) (*DecodedFrame, error) {
	frame, err := d.DecodeFrame(frameData)
	if err != nil {
		d.stats.DroppedFrames++
		return nil, err
	}

	d.stats.TotalFrames++
	d.stats.TotalBytes += uint64(len(frameData))
	if frame.IsKeyframe {
		d.stats.KeyFrames++
	}

	if d.stats.TotalFrames > 0 {
		d.stats.AverageFrameSize = d.stats.TotalBytes / d.stats.TotalFrames
	}

	return frame, nil
}

func (d *VideoDecoderWithStats) GetStatistics() FrameStatistics {
	return d.stats
}

func (d *VideoDecoderWithStats) ResetStatistics() {
	d.stats = FrameStatistics{}
}

func readUint32LE(data []byte) uint32 {
	return binary.LittleEndian.Uint32(data)
}

func writeUint32LE(value uint32) []byte {
	buf := make([]byte, 4)
	binary.LittleEndian.PutUint32(buf, value)
	return buf
}

func ExtractResolutionFromKeyframe(codec VideoCodec, frameData []byte) (width, height int, err error) {
	switch codec {
	case CodecVP8:
		return extractVP8Resolution(frameData)
	case CodecVP9:
		return extractVP9Resolution(frameData)
	default:
		return 0, 0, fmt.Errorf("resolution extraction not supported for codec")
	}
}

func extractVP8Resolution(data []byte) (width, height int, err error) {
	if len(data) < 10 {
		return 0, 0, fmt.Errorf("VP8 keyframe too small")
	}

	if (data[0] & 0x01) != 0 {
		return 0, 0, fmt.Errorf("not a VP8 keyframe")
	}

	if data[3] != 0x9d || data[4] != 0x01 || data[5] != 0x2a {
		return 0, 0, fmt.Errorf("invalid VP8 start code")
	}

	width = int(binary.LittleEndian.Uint16(data[6:8]) & 0x3FFF)
	height = int(binary.LittleEndian.Uint16(data[8:10]) & 0x3FFF)

	return width, height, nil
}

func extractVP9Resolution(data []byte) (width, height int, err error) {
	if len(data) < 2 {
		return 0, 0, fmt.Errorf("VP9 frame too small")
	}

	if (data[0] & 0x01) != 0 {
		return 0, 0, fmt.Errorf("not a VP9 keyframe")
	}

	return 0, 0, fmt.Errorf("VP9 resolution extraction requires full parser")
}

func ExampleDecoderUsage() {

	decoder := NewVideoDecoderWithStats(CodecVP9, 1920, 1080)

	encodedFrame := make([]byte, 1000)

	decodedFrame, err := decoder.DecodeFrameWithStats(encodedFrame)
	if err != nil {
		log.Printf("Decode error: %v", err)
		return
	}

	log.Printf("Decoded frame: %dx%d, keyframe=%v, format=%s",
		decodedFrame.Width, decodedFrame.Height,
		decodedFrame.IsKeyframe, decodedFrame.Format)

	stats := decoder.GetStatistics()
	log.Printf("Stats: %d total frames, %d keyframes, avg size: %d bytes",
		stats.TotalFrames, stats.KeyFrames, stats.AverageFrameSize)
}
