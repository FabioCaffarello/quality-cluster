package memdb

import (
	"context"
	"errors"
	"strconv"
	"sync"
	"testing"
)

func TestDBPutGetDeleteWithDefensiveCopies(t *testing.T) {
	t.Parallel()

	db := New()
	source := []byte("value-1")
	if err := db.Update(context.Background(), func(tx WriteTx) error {
		tx.Put("configs", "id-1", source)
		return nil
	}); err != nil {
		t.Fatalf("update: %v", err)
	}

	source[0] = 'X'

	if err := db.View(context.Background(), func(tx ReadTx) error {
		value, ok := tx.Get("configs", "id-1")
		if !ok {
			t.Fatal("expected value")
		}
		if string(value) != "value-1" {
			t.Fatalf("expected stored value to be isolated, got %q", string(value))
		}

		value[0] = 'Y'
		again, ok := tx.Get("configs", "id-1")
		if !ok {
			t.Fatal("expected value on second read")
		}
		if string(again) != "value-1" {
			t.Fatalf("expected read copy to be isolated, got %q", string(again))
		}
		return nil
	}); err != nil {
		t.Fatalf("view: %v", err)
	}

	if err := db.Update(context.Background(), func(tx WriteTx) error {
		tx.Delete("configs", "id-1")
		return nil
	}); err != nil {
		t.Fatalf("delete: %v", err)
	}

	if err := db.View(context.Background(), func(tx ReadTx) error {
		if _, ok := tx.Get("configs", "id-1"); ok {
			t.Fatal("expected deleted key to be absent")
		}
		return nil
	}); err != nil {
		t.Fatalf("view after delete: %v", err)
	}
}

func TestDBListAndScanPrefixAreSortedAndDefensive(t *testing.T) {
	t.Parallel()

	db := New()
	if err := db.Update(context.Background(), func(tx WriteTx) error {
		tx.Put("index", "ver-2\x00act-2", []byte("2"))
		tx.Put("index", "ver-1\x00act-1", []byte("1"))
		tx.Put("index", "ver-1\x00act-3", []byte("3"))
		return nil
	}); err != nil {
		t.Fatalf("seed: %v", err)
	}

	if err := db.View(context.Background(), func(tx ReadTx) error {
		all := tx.List("index")
		if len(all) != 3 {
			t.Fatalf("expected 3 entries, got %d", len(all))
		}
		if all[0].Key != "ver-1\x00act-1" || all[1].Key != "ver-1\x00act-3" || all[2].Key != "ver-2\x00act-2" {
			t.Fatalf("expected sorted keys, got %+v", all)
		}

		all[0].Value[0] = '9'

		scanned := tx.ScanPrefix("index", "ver-1\x00")
		if len(scanned) != 2 {
			t.Fatalf("expected 2 prefixed entries, got %d", len(scanned))
		}
		if string(scanned[0].Value) != "1" {
			t.Fatalf("expected scan to return defensive copies, got %q", string(scanned[0].Value))
		}
		return nil
	}); err != nil {
		t.Fatalf("view: %v", err)
	}
}

func TestDBUpdateProvidesAtomicReadModifyWrite(t *testing.T) {
	t.Parallel()

	db := New()
	const increments = 64

	var wg sync.WaitGroup
	for range increments {
		wg.Add(1)
		go func() {
			defer wg.Done()

			if err := db.Update(context.Background(), func(tx WriteTx) error {
				value, ok := tx.Get("counters", "hits")
				current := 0
				if ok {
					parsed, err := strconv.Atoi(string(value))
					if err != nil {
						return err
					}
					current = parsed
				}
				tx.Put("counters", "hits", []byte(strconv.Itoa(current+1)))
				return nil
			}); err != nil {
				t.Errorf("update: %v", err)
			}
		}()
	}
	wg.Wait()

	if err := db.View(context.Background(), func(tx ReadTx) error {
		value, ok := tx.Get("counters", "hits")
		if !ok {
			t.Fatal("expected counter value")
		}
		if string(value) != strconv.Itoa(increments) {
			t.Fatalf("expected %d, got %s", increments, string(value))
		}
		return nil
	}); err != nil {
		t.Fatalf("view: %v", err)
	}
}

func TestDBUpdateRollsBackWhenCallbackFails(t *testing.T) {
	t.Parallel()

	db := New()
	if err := db.Update(context.Background(), func(tx WriteTx) error {
		tx.Put("configs", "id-1", []byte("before"))
		return nil
	}); err != nil {
		t.Fatalf("seed: %v", err)
	}

	expectedErr := errors.New("boom")
	err := db.Update(context.Background(), func(tx WriteTx) error {
		tx.Put("configs", "id-1", []byte("after"))
		tx.Put("configs", "id-2", []byte("new"))
		tx.Delete("configs", "id-1")
		return expectedErr
	})
	if !errors.Is(err, expectedErr) {
		t.Fatalf("expected %v, got %v", expectedErr, err)
	}

	if err := db.View(context.Background(), func(tx ReadTx) error {
		value, ok := tx.Get("configs", "id-1")
		if !ok || string(value) != "before" {
			t.Fatalf("expected rolled back value, got ok=%v value=%q", ok, string(value))
		}
		if _, ok := tx.Get("configs", "id-2"); ok {
			t.Fatal("expected new key to be rolled back")
		}
		return nil
	}); err != nil {
		t.Fatalf("view: %v", err)
	}
}

func TestDBHonorsContextBeforeRunningCallbacks(t *testing.T) {
	t.Parallel()

	db := New()
	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	calledView := false
	if err := db.View(ctx, func(ReadTx) error {
		calledView = true
		return nil
	}); err == nil {
		t.Fatal("expected canceled view to fail")
	}
	if calledView {
		t.Fatal("expected view callback to be skipped")
	}

	calledUpdate := false
	if err := db.Update(ctx, func(WriteTx) error {
		calledUpdate = true
		return nil
	}); err == nil {
		t.Fatal("expected canceled update to fail")
	}
	if calledUpdate {
		t.Fatal("expected update callback to be skipped")
	}
}

func TestDBAbortsCommitIfContextIsCanceledDuringUpdate(t *testing.T) {
	t.Parallel()

	db := New()
	ctx, cancel := context.WithCancel(context.Background())

	err := db.Update(ctx, func(tx WriteTx) error {
		tx.Put("configs", "id-1", []byte("value-1"))
		cancel()
		return nil
	})
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("expected context canceled, got %v", err)
	}

	if err := db.View(context.Background(), func(tx ReadTx) error {
		if _, ok := tx.Get("configs", "id-1"); ok {
			t.Fatal("expected canceled update to avoid commit")
		}
		return nil
	}); err != nil {
		t.Fatalf("view: %v", err)
	}
}

func TestDBRejectsNilReceiverAndNilCallback(t *testing.T) {
	t.Parallel()

	var db *DB
	if err := db.View(context.Background(), func(ReadTx) error { return nil }); !errors.Is(err, ErrNilDB) {
		t.Fatalf("expected ErrNilDB from View, got %v", err)
	}
	if err := db.Update(context.Background(), func(WriteTx) error { return nil }); !errors.Is(err, ErrNilDB) {
		t.Fatalf("expected ErrNilDB from Update, got %v", err)
	}

	db = New()
	if err := db.View(context.Background(), nil); !errors.Is(err, ErrNilCallback) {
		t.Fatalf("expected ErrNilCallback from View, got %v", err)
	}
	if err := db.Update(context.Background(), nil); !errors.Is(err, ErrNilCallback) {
		t.Fatalf("expected ErrNilCallback from Update, got %v", err)
	}
}
