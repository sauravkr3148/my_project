package relay

import (
	"sync"
	"time"
)

type FrameBuffer struct {
	frames   []VideoFrame
	maxSize  int
	mu       sync.RWMutex
	lastPush time.Time
	dropped  uint64
	sent     uint64
}

type VideoFrame struct {
	Data      []byte
	Timestamp time.Time
	IsKey     bool
	FrameID   uint64
}

func NewFrameBuffer(maxSize int) *FrameBuffer {
	if maxSize <= 0 {
		maxSize = 30
	}
	return &FrameBuffer{
		frames:   make([]VideoFrame, 0, maxSize),
		maxSize:  maxSize,
		lastPush: time.Now(),
	}
}

func (fb *FrameBuffer) Push(data []byte, isKey bool) {
	fb.mu.Lock()
	defer fb.mu.Unlock()

	if len(fb.frames) >= fb.maxSize {
		if !isKey {
			fb.dropped++
			return
		}
		oldestNonKey := -1
		for i := 0; i < len(fb.frames); i++ {
			if !fb.frames[i].IsKey {
				oldestNonKey = i
				break
			}
		}
		if oldestNonKey != -1 {
			fb.frames = append(fb.frames[:oldestNonKey], fb.frames[oldestNonKey+1:]...)
		} else {
			fb.frames = fb.frames[1:]
		}
		fb.dropped++
	}

	frameID := fb.sent + fb.dropped
	frame := VideoFrame{
		Data:      data,
		Timestamp: time.Now(),
		IsKey:     isKey,
		FrameID:   frameID,
	}

	fb.frames = append(fb.frames, frame)
	fb.lastPush = time.Now()
}

func (fb *FrameBuffer) Pop() (VideoFrame, bool) {
	fb.mu.Lock()
	defer fb.mu.Unlock()

	if len(fb.frames) == 0 {
		return VideoFrame{}, false
	}

	frame := fb.frames[0]
	fb.frames = fb.frames[1:]
	fb.sent++
	return frame, true
}

func (fb *FrameBuffer) PopAll() []VideoFrame {
	fb.mu.Lock()
	defer fb.mu.Unlock()

	if len(fb.frames) == 0 {
		return nil
	}

	frames := make([]VideoFrame, len(fb.frames))
	copy(frames, fb.frames)
	fb.sent += uint64(len(frames))
	fb.frames = fb.frames[:0]
	return frames
}

func (fb *FrameBuffer) Peek() (VideoFrame, bool) {
	fb.mu.RLock()
	defer fb.mu.RUnlock()

	if len(fb.frames) == 0 {
		return VideoFrame{}, false
	}

	return fb.frames[0], true
}

func (fb *FrameBuffer) Size() int {
	fb.mu.RLock()
	defer fb.mu.RUnlock()
	return len(fb.frames)
}

func (fb *FrameBuffer) Clear() {
	fb.mu.Lock()
	defer fb.mu.Unlock()
	fb.frames = fb.frames[:0]
}

func (fb *FrameBuffer) GetStats() FrameBufferStats {
	fb.mu.RLock()
	defer fb.mu.RUnlock()

	return FrameBufferStats{
		CurrentSize: len(fb.frames),
		MaxSize:     fb.maxSize,
		Dropped:     fb.dropped,
		Sent:        fb.sent,
		LastPush:    fb.lastPush,
	}
}

func (fb *FrameBuffer) HasKeyFrame() bool {
	fb.mu.RLock()
	defer fb.mu.RUnlock()

	for i := range fb.frames {
		if fb.frames[i].IsKey {
			return true
		}
	}
	return false
}

func (fb *FrameBuffer) GetKeyFrame() (VideoFrame, bool) {
	fb.mu.RLock()
	defer fb.mu.RUnlock()

	for i := range fb.frames {
		if fb.frames[i].IsKey {
			return fb.frames[i], true
		}
	}
	return VideoFrame{}, false
}

func (fb *FrameBuffer) DropOldFrames(maxAge time.Duration) int {
	fb.mu.Lock()
	defer fb.mu.Unlock()

	cutoff := time.Now().Add(-maxAge)
	newStart := 0

	for i, frame := range fb.frames {
		if frame.Timestamp.After(cutoff) {
			newStart = i
			break
		}
	}

	if newStart > 0 {
		fb.frames = fb.frames[newStart:]
		fb.dropped += uint64(newStart)
		return newStart
	}

	return 0
}

func (fb *FrameBuffer) GetFramesAfter(timestamp time.Time) []VideoFrame {
	fb.mu.RLock()
	defer fb.mu.RUnlock()

	var result []VideoFrame
	for _, frame := range fb.frames {
		if frame.Timestamp.After(timestamp) {
			result = append(result, frame)
		}
	}
	return result
}

type FrameBufferStats struct {
	CurrentSize int
	MaxSize     int
	Dropped     uint64
	Sent        uint64
	LastPush    time.Time
}

type ClientFrameBuffer struct {
	buffers map[string]*FrameBuffer
	mu      sync.RWMutex
}

func NewClientFrameBuffer() *ClientFrameBuffer {
	return &ClientFrameBuffer{
		buffers: make(map[string]*FrameBuffer),
	}
}

func (cfb *ClientFrameBuffer) GetOrCreate(clientID string, maxSize int) *FrameBuffer {
	cfb.mu.Lock()
	defer cfb.mu.Unlock()

	if buffer, exists := cfb.buffers[clientID]; exists {
		return buffer
	}

	buffer := NewFrameBuffer(maxSize)
	cfb.buffers[clientID] = buffer
	return buffer
}

func (cfb *ClientFrameBuffer) Remove(clientID string) {
	cfb.mu.Lock()
	defer cfb.mu.Unlock()
	delete(cfb.buffers, clientID)
}

func (cfb *ClientFrameBuffer) GetStats(clientID string) (FrameBufferStats, bool) {
	cfb.mu.RLock()
	defer cfb.mu.RUnlock()

	if buffer, exists := cfb.buffers[clientID]; exists {
		return buffer.GetStats(), true
	}
	return FrameBufferStats{}, false
}

func (cfb *ClientFrameBuffer) CleanupStale(maxAge time.Duration) int {
	cfb.mu.Lock()
	defer cfb.mu.Unlock()

	cutoff := time.Now().Add(-maxAge)
	removed := 0

	for clientID, buffer := range cfb.buffers {
		buffer.mu.RLock()
		lastPush := buffer.lastPush
		buffer.mu.RUnlock()

		if lastPush.Before(cutoff) {
			delete(cfb.buffers, clientID)
			removed++
		}
	}
	return removed
}
