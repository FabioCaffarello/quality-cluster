package nats

import (
	"fmt"
	"strings"
	"time"

	"github.com/nats-io/nats.go/jetstream"
)

type StreamType string

const (
	// from server to configctl
	StreamTypeConfigctl       StreamType = "config"

	// from exchange processors to store
	StreamTypeStoreConfigctl  StreamType = "store_config"

	StreamTypeWildcard StreamType = "*"
)

// get the config based on the stream type
func (s StreamType) GetStreamConfig() jetstream.StreamConfig {
	switch s {
	// realtime unfinal data streams
	// we dont need the queue to retain the data
	// as this goes directly to clients and does not need replay
	// so it used memory storage with 128MB max size
	case StreamTypeConfigctl:
		return jetstream.StreamConfig{
			MaxBytes: 1024 * 1024 * 128, // 128MB
			Storage:  jetstream.MemoryStorage,
			MaxAge:   time.Minute * 5,
		}

	// store final data streams
	// we need the queue to retain the data so that it can
	// be replayed to the store if it goes down
	// so it used file storage with 2GB max size
	case StreamTypeStoreConfigctl:
		return jetstream.StreamConfig{
			MaxBytes: 1024 * 1024 * 1024 * 2, // 2GB
			Storage:  jetstream.FileStorage,
			MaxAge:   time.Hour * 12,
		}

	default:
		return jetstream.StreamConfig{}
	}
}

func (s StreamType) IsValid() bool {
	switch s {
	case StreamTypeConfigctl,
		StreamTypeStoreConfigctl:
		return true
	default:
		return false
	}
}

type Subject struct {
	StreamType StreamType
	SubjectTarget   string
	Timeframe  int64
}

// get the subscribe string
// subscribe can have wildcards
func (s Subject) SubString() string {
	if s.StreamType == "" {
		s.StreamType = "*"
	}

	lowerSubjectTarget := strings.ToLower(s.SubjectTarget)
	if s.Timeframe != 0 {
		if lowerSubjectTarget == "" {
			lowerSubjectTarget = "*"
		}
		return fmt.Sprintf("%s.%s.%d", s.StreamType, lowerSubjectTarget, s.Timeframe)
	}

	if lowerSubjectTarget != "" {
		return fmt.Sprintf("%s.%s.>", s.StreamType, lowerSubjectTarget)
	}
	return fmt.Sprintf("%s.>", s.StreamType)
}

// get the publish string
// publish strings do not have wildcards
func (s Subject) PubString() string {
	if s.StreamType == "" || s.SubjectTarget == "" {
		return ""
	}

	lowerSubjectTarget := strings.ToLower(s.SubjectTarget)
	if s.Timeframe != 0 {
		return fmt.Sprintf("%s.%s.%d", s.StreamType, lowerSubjectTarget, s.Timeframe)
	}
	return fmt.Sprintf("%s.%s", s.StreamType, lowerSubjectTarget)
}

// Name of the stream
func (s StreamType) Name() string {
	return string(s)
}

// Durable name of the stream
func (s StreamType) Durable(name string) string {
	nameUpper := strings.ToUpper(name)
	return fmt.Sprintf("%s:%s", s, nameUpper)
}

// Config of the stream
func (s StreamType) Config() jetstream.StreamConfig {
	config := s.GetStreamConfig()
	return jetstream.StreamConfig{
		Name:       s.Name(),
		Subjects:   []string{Subject{StreamType: s}.SubString()},
		MaxAge:     config.MaxAge,
		MaxBytes:   config.MaxBytes,
		Storage:    config.Storage,
		MaxMsgSize: 1024 * 1024 * 10, // 10MB
	}
}
