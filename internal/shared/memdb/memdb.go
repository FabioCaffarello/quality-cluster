package memdb

import (
	"context"
	"errors"
	"sort"
	"strings"
	"sync"
)

var (
	ErrNilDB       = errors.New("memdb: nil DB")
	ErrNilCallback = errors.New("memdb: nil callback")
)

type Entry struct {
	Key   string
	Value []byte
}

type ReadTx interface {
	Get(bucket, key string) ([]byte, bool)
	List(bucket string) []Entry
	ScanPrefix(bucket, prefix string) []Entry
}

type WriteTx interface {
	ReadTx
	Put(bucket, key string, value []byte)
	Delete(bucket, key string)
}

type DB struct {
	mu      sync.RWMutex
	buckets map[string]map[string][]byte
}

func New() *DB {
	return &DB{
		buckets: make(map[string]map[string][]byte),
	}
}

func (db *DB) View(ctx context.Context, fn func(ReadTx) error) error {
	if db == nil {
		return ErrNilDB
	}
	if fn == nil {
		return ErrNilCallback
	}
	if err := ctxErr(ctx); err != nil {
		return err
	}

	db.mu.RLock()
	defer db.mu.RUnlock()

	if err := ctxErr(ctx); err != nil {
		return err
	}

	return fn(readTx{buckets: db.buckets})
}

func (db *DB) Update(ctx context.Context, fn func(WriteTx) error) error {
	if db == nil {
		return ErrNilDB
	}
	if fn == nil {
		return ErrNilCallback
	}
	if err := ctxErr(ctx); err != nil {
		return err
	}

	db.mu.Lock()
	defer db.mu.Unlock()

	if err := ctxErr(ctx); err != nil {
		return err
	}

	working := cloneBuckets(db.buckets)
	if err := fn(writeTx{buckets: working}); err != nil {
		return err
	}
	if err := ctxErr(ctx); err != nil {
		return err
	}

	db.buckets = working
	return nil
}

type readTx struct {
	buckets map[string]map[string][]byte
}

func (tx readTx) Get(bucket, key string) ([]byte, bool) {
	records, ok := tx.buckets[bucket]
	if !ok {
		return nil, false
	}
	value, ok := records[key]
	if !ok {
		return nil, false
	}
	return cloneBytes(value), true
}

func (tx readTx) List(bucket string) []Entry {
	return tx.scan(bucket, "")
}

func (tx readTx) ScanPrefix(bucket, prefix string) []Entry {
	return tx.scan(bucket, prefix)
}

func (tx readTx) scan(bucket, prefix string) []Entry {
	records, ok := tx.buckets[bucket]
	if !ok {
		return nil
	}

	entries := make([]Entry, 0, len(records))
	for key, value := range records {
		if prefix != "" && !strings.HasPrefix(key, prefix) {
			continue
		}
		entries = append(entries, Entry{
			Key:   key,
			Value: cloneBytes(value),
		})
	}

	sort.Slice(entries, func(i, j int) bool {
		return entries[i].Key < entries[j].Key
	})

	return entries
}

type writeTx struct {
	buckets map[string]map[string][]byte
}

func (tx writeTx) Get(bucket, key string) ([]byte, bool) {
	return readTx(tx).Get(bucket, key)
}

func (tx writeTx) List(bucket string) []Entry {
	return readTx(tx).List(bucket)
}

func (tx writeTx) ScanPrefix(bucket, prefix string) []Entry {
	return readTx(tx).ScanPrefix(bucket, prefix)
}

func (tx writeTx) Put(bucket, key string, value []byte) {
	records, ok := tx.buckets[bucket]
	if !ok {
		records = make(map[string][]byte)
		tx.buckets[bucket] = records
	}
	records[key] = cloneBytes(value)
}

func (tx writeTx) Delete(bucket, key string) {
	records, ok := tx.buckets[bucket]
	if !ok {
		return
	}
	delete(records, key)
	if len(records) == 0 {
		delete(tx.buckets, bucket)
	}
}

func cloneBytes(value []byte) []byte {
	if value == nil {
		return nil
	}
	cloned := make([]byte, len(value))
	copy(cloned, value)
	return cloned
}

func cloneBuckets(source map[string]map[string][]byte) map[string]map[string][]byte {
	if len(source) == 0 {
		return make(map[string]map[string][]byte)
	}

	cloned := make(map[string]map[string][]byte, len(source))
	for bucket, records := range source {
		bucketCopy := make(map[string][]byte, len(records))
		for key, value := range records {
			bucketCopy[key] = cloneBytes(value)
		}
		cloned[bucket] = bucketCopy
	}

	return cloned
}

func ctxErr(ctx context.Context) error {
	if ctx == nil {
		return nil
	}
	return ctx.Err()
}
