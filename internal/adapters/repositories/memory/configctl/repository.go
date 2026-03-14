package configctl

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"sort"

	configdomain "internal/domain/configctl"
	"internal/shared/memdb"
	"internal/shared/problem"
)

const (
	bucketConfigSets           = "config_sets"
	bucketConfigSetByKey       = "config_sets_by_key"
	bucketConfigSetByVersionID = "config_sets_by_version_id"
	bucketActivations          = "activations"
	bucketActivationByScope    = "activations_by_scope"
	bucketActivationByVersion  = "activations_by_version_id"
	bucketIngestionRuntimes    = "ingestion_runtimes"
)

const activationVersionSeparator = "\x00"

type Repository struct {
	db *memdb.DB
}

func NewRepository(db *memdb.DB) *Repository {
	if db == nil {
		db = memdb.New()
	}
	return &Repository{db: db}
}

func (r *Repository) SaveConfigSet(ctx context.Context, set configdomain.ConfigSet) *problem.Problem {
	if r == nil || r.db == nil {
		return problem.New(problem.Unavailable, "config repository is unavailable")
	}

	record := newConfigSetRecord(set)
	payload, err := encodeConfigSetRecord(record)
	if err != nil {
		return problem.Wrap(err, problem.Internal, "encode config set record")
	}

	err = r.db.Update(ctx, func(tx memdb.WriteTx) error {
		existing, ok, err := loadConfigSetRecord(tx, set.ID)
		if err != nil {
			return err
		}
		if ok {
			removeStaleConfigSetIndexes(tx, existing, record)
		}

		tx.Put(bucketConfigSets, record.ID, payload)
		tx.Put(bucketConfigSetByKey, record.Key, []byte(record.ID))
		for _, version := range record.Versions {
			tx.Put(bucketConfigSetByVersionID, version.ID, []byte(record.ID))
		}
		return nil
	})
	return mapStorageErr(err, "save config set")
}

func (r *Repository) DeleteConfigSet(ctx context.Context, id string) *problem.Problem {
	if r == nil || r.db == nil {
		return problem.New(problem.Unavailable, "config repository is unavailable")
	}

	err := r.db.Update(ctx, func(tx memdb.WriteTx) error {
		record, ok, err := loadConfigSetRecord(tx, id)
		if err != nil {
			return err
		}
		if !ok {
			return nil
		}

		tx.Delete(bucketConfigSets, id)
		tx.Delete(bucketConfigSetByKey, record.Key)
		for _, version := range record.Versions {
			tx.Delete(bucketConfigSetByVersionID, version.ID)
		}
		return nil
	})
	return mapStorageErr(err, "delete config set")
}

func (r *Repository) GetConfigSetByID(ctx context.Context, id string) (configdomain.ConfigSet, *problem.Problem) {
	if r == nil || r.db == nil {
		return configdomain.ConfigSet{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	var set configdomain.ConfigSet
	err := r.db.View(ctx, func(tx memdb.ReadTx) error {
		record, ok, err := loadConfigSetRecord(tx, id)
		if err != nil {
			return err
		}
		if !ok {
			return errConfigSetNotFound
		}
		set, err = record.toDomain()
		if err != nil {
			return err
		}
		return nil
	})
	if err != nil {
		if errors.Is(err, errConfigSetNotFound) {
			return configdomain.ConfigSet{}, problem.New(problem.NotFound, "config set not found")
		}
		return configdomain.ConfigSet{}, mapStorageErr(err, "get config set")
	}

	return set, nil
}

func (r *Repository) GetConfigSetByKey(ctx context.Context, key string) (configdomain.ConfigSet, *problem.Problem) {
	if r == nil || r.db == nil {
		return configdomain.ConfigSet{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	return r.getConfigSetByIndex(ctx, bucketConfigSetByKey, key, "config set not found")
}

func (r *Repository) GetConfigSetByVersionID(ctx context.Context, versionID string) (configdomain.ConfigSet, *problem.Problem) {
	if r == nil || r.db == nil {
		return configdomain.ConfigSet{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	return r.getConfigSetByIndex(ctx, bucketConfigSetByVersionID, versionID, "config version not found")
}

func (r *Repository) ListConfigSets(ctx context.Context) ([]configdomain.ConfigSet, *problem.Problem) {
	if r == nil || r.db == nil {
		return nil, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	sets := make([]configdomain.ConfigSet, 0)
	err := r.db.View(ctx, func(tx memdb.ReadTx) error {
		for _, entry := range tx.List(bucketConfigSets) {
			record, err := decodeConfigSetRecord(entry.Value)
			if err != nil {
				return err
			}
			set, err := record.toDomain()
			if err != nil {
				return err
			}
			sets = append(sets, set)
		}
		return nil
	})
	if err != nil {
		return nil, mapStorageErr(err, "list config sets")
	}

	sort.SliceStable(sets, func(i, j int) bool {
		if sets[i].CreatedAt.Equal(sets[j].CreatedAt) {
			return sets[i].ID < sets[j].ID
		}
		return sets[i].CreatedAt.Before(sets[j].CreatedAt)
	})

	return sets, nil
}

func (r *Repository) SaveActivation(ctx context.Context, activation configdomain.Activation) *problem.Problem {
	if r == nil || r.db == nil {
		return problem.New(problem.Unavailable, "config repository is unavailable")
	}

	record := newActivationRecord(activation)
	payload, err := encodeActivationRecord(record)
	if err != nil {
		return problem.Wrap(err, problem.Internal, "encode activation record")
	}

	err = r.db.Update(ctx, func(tx memdb.WriteTx) error {
		existing, ok, err := loadActivationRecord(tx, activation.ID)
		if err != nil {
			return err
		}
		if ok {
			removeStaleActivationIndexes(tx, existing, record)
		}

		tx.Put(bucketActivations, record.ID, payload)
		tx.Put(bucketActivationByVersion, activationVersionIndexKey(record.VersionID, record.ID), []byte(record.ID))
		if record.isActive() {
			tx.Put(bucketActivationByScope, scopeIndexKey(record.Scope), []byte(record.ID))
		} else {
			deleteScopeIndexIfMatches(tx, record.Scope, record.ID)
		}
		return nil
	})
	return mapStorageErr(err, "save activation")
}

func (r *Repository) DeleteActivation(ctx context.Context, activationID string) *problem.Problem {
	if r == nil || r.db == nil {
		return problem.New(problem.Unavailable, "config repository is unavailable")
	}

	err := r.db.Update(ctx, func(tx memdb.WriteTx) error {
		record, ok, err := loadActivationRecord(tx, activationID)
		if err != nil {
			return err
		}
		if !ok {
			return nil
		}

		tx.Delete(bucketActivations, activationID)
		tx.Delete(bucketActivationByVersion, activationVersionIndexKey(record.VersionID, record.ID))
		if record.isActive() {
			deleteScopeIndexIfMatches(tx, record.Scope, record.ID)
		}
		return nil
	})
	return mapStorageErr(err, "delete activation")
}

func (r *Repository) GetActivationByScope(ctx context.Context, scope configdomain.ActivationScope) (configdomain.Activation, *problem.Problem) {
	if r == nil || r.db == nil {
		return configdomain.Activation{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	var activation configdomain.Activation
	err := r.db.View(ctx, func(tx memdb.ReadTx) error {
		activationID, ok := tx.Get(bucketActivationByScope, scope.Normalize().String())
		if !ok {
			return errActivationNotFound
		}

		record, ok, err := loadActivationRecord(tx, string(activationID))
		if err != nil {
			return err
		}
		if !ok {
			return errCorruptedIndex
		}

		activation, err = record.toDomain()
		if err != nil {
			return err
		}
		return nil
	})
	if err != nil {
		switch {
		case errors.Is(err, errActivationNotFound):
			return configdomain.Activation{}, problem.New(problem.NotFound, "activation not found")
		case errors.Is(err, errCorruptedIndex):
			return configdomain.Activation{}, problem.New(problem.Internal, "activation index is corrupted")
		default:
			return configdomain.Activation{}, mapStorageErr(err, "get activation")
		}
	}

	return activation, nil
}

func (r *Repository) ListActivationsByVersionID(ctx context.Context, versionID string) ([]configdomain.Activation, *problem.Problem) {
	if r == nil || r.db == nil {
		return nil, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	activations := make([]configdomain.Activation, 0)
	err := r.db.View(ctx, func(tx memdb.ReadTx) error {
		prefix := versionID + activationVersionSeparator
		for _, entry := range tx.ScanPrefix(bucketActivationByVersion, prefix) {
			record, ok, err := loadActivationRecord(tx, string(entry.Value))
			if err != nil {
				return err
			}
			if !ok {
				return errCorruptedIndex
			}
			activation, err := record.toDomain()
			if err != nil {
				return err
			}
			activations = append(activations, activation)
		}
		return nil
	})
	if err != nil {
		if errors.Is(err, errCorruptedIndex) {
			return nil, problem.New(problem.Internal, "activation index is corrupted")
		}
		return nil, mapStorageErr(err, "list activations")
	}

	sort.SliceStable(activations, func(i, j int) bool {
		if activations[i].ActivatedAt.Equal(activations[j].ActivatedAt) {
			return activations[i].ID < activations[j].ID
		}
		return activations[i].ActivatedAt.Before(activations[j].ActivatedAt)
	})

	return activations, nil
}

func (r *Repository) SaveIngestionRuntime(ctx context.Context, runtime configdomain.IngestionRuntimeProjection) *problem.Problem {
	if r == nil || r.db == nil {
		return problem.New(problem.Unavailable, "config repository is unavailable")
	}

	record := newIngestionRuntimeRecord(runtime)
	payload, err := encodeIngestionRuntimeRecord(record)
	if err != nil {
		return problem.Wrap(err, problem.Internal, "encode ingestion runtime record")
	}

	err = r.db.Update(ctx, func(tx memdb.WriteTx) error {
		tx.Put(bucketIngestionRuntimes, scopeIndexKey(record.Scope), payload)
		return nil
	})
	return mapStorageErr(err, "save ingestion runtime")
}

func (r *Repository) DeleteIngestionRuntimeByScope(ctx context.Context, scope configdomain.ActivationScope) *problem.Problem {
	if r == nil || r.db == nil {
		return problem.New(problem.Unavailable, "config repository is unavailable")
	}

	err := r.db.Update(ctx, func(tx memdb.WriteTx) error {
		tx.Delete(bucketIngestionRuntimes, scope.Normalize().String())
		return nil
	})
	return mapStorageErr(err, "delete ingestion runtime")
}

func (r *Repository) ListIngestionRuntimes(ctx context.Context) ([]configdomain.IngestionRuntimeProjection, *problem.Problem) {
	if r == nil || r.db == nil {
		return nil, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	runtimes := make([]configdomain.IngestionRuntimeProjection, 0)
	err := r.db.View(ctx, func(tx memdb.ReadTx) error {
		for _, entry := range tx.List(bucketIngestionRuntimes) {
			record, err := decodeIngestionRuntimeRecord(entry.Value)
			if err != nil {
				return err
			}
			runtime, err := record.toDomain()
			if err != nil {
				return err
			}
			runtimes = append(runtimes, runtime)
		}
		return nil
	})
	if err != nil {
		return nil, mapStorageErr(err, "list ingestion runtimes")
	}

	sort.SliceStable(runtimes, func(i, j int) bool {
		left := runtimes[i].Scope.Normalize().String()
		right := runtimes[j].Scope.Normalize().String()
		if left == right {
			return runtimes[i].ActivatedAt.Before(runtimes[j].ActivatedAt)
		}
		return left < right
	})

	return runtimes, nil
}

var (
	errConfigSetNotFound  = errors.New("config set not found")
	errActivationNotFound = errors.New("activation not found")
	errCorruptedIndex     = errors.New("corrupted index")
)

func (r *Repository) getConfigSetByIndex(ctx context.Context, bucket, indexKey, notFoundMessage string) (configdomain.ConfigSet, *problem.Problem) {
	var set configdomain.ConfigSet
	err := r.db.View(ctx, func(tx memdb.ReadTx) error {
		setID, ok := tx.Get(bucket, indexKey)
		if !ok {
			return errConfigSetNotFound
		}

		record, ok, err := loadConfigSetRecord(tx, string(setID))
		if err != nil {
			return err
		}
		if !ok {
			return errCorruptedIndex
		}

		set, err = record.toDomain()
		if err != nil {
			return err
		}
		return nil
	})
	if err != nil {
		switch {
		case errors.Is(err, errConfigSetNotFound):
			return configdomain.ConfigSet{}, problem.New(problem.NotFound, notFoundMessage)
		case errors.Is(err, errCorruptedIndex):
			return configdomain.ConfigSet{}, problem.New(problem.Internal, "config set index is corrupted")
		default:
			return configdomain.ConfigSet{}, mapStorageErr(err, "get config set")
		}
	}

	return set, nil
}

func loadConfigSetRecord(tx memdb.ReadTx, id string) (configSetRecord, bool, error) {
	payload, ok := tx.Get(bucketConfigSets, id)
	if !ok {
		return configSetRecord{}, false, nil
	}
	record, err := decodeConfigSetRecord(payload)
	if err != nil {
		return configSetRecord{}, false, err
	}
	return record, true, nil
}

func loadActivationRecord(tx memdb.ReadTx, id string) (activationRecord, bool, error) {
	payload, ok := tx.Get(bucketActivations, id)
	if !ok {
		return activationRecord{}, false, nil
	}
	record, err := decodeActivationRecord(payload)
	if err != nil {
		return activationRecord{}, false, err
	}
	return record, true, nil
}

func removeStaleConfigSetIndexes(tx memdb.WriteTx, existing, next configSetRecord) {
	if existing.Key != next.Key {
		tx.Delete(bucketConfigSetByKey, existing.Key)
	}

	nextVersions := make(map[string]struct{}, len(next.Versions))
	for _, version := range next.Versions {
		nextVersions[version.ID] = struct{}{}
	}
	for _, version := range existing.Versions {
		if _, keep := nextVersions[version.ID]; !keep {
			tx.Delete(bucketConfigSetByVersionID, version.ID)
		}
	}
}

func removeStaleActivationIndexes(tx memdb.WriteTx, existing, next activationRecord) {
	if existing.VersionID != next.VersionID {
		tx.Delete(bucketActivationByVersion, activationVersionIndexKey(existing.VersionID, existing.ID))
	}

	existingScope := scopeIndexKey(existing.Scope)
	nextScope := scopeIndexKey(next.Scope)
	if existing.isActive() && (!next.isActive() || existingScope != nextScope) {
		deleteScopeIndexIfMatches(tx, existing.Scope, existing.ID)
	}
}

func deleteScopeIndexIfMatches(tx memdb.WriteTx, scope activationScopeRecord, activationID string) {
	current, ok := tx.Get(bucketActivationByScope, scopeIndexKey(scope))
	if ok && string(current) == activationID {
		tx.Delete(bucketActivationByScope, scopeIndexKey(scope))
	}
}

func activationVersionIndexKey(versionID, activationID string) string {
	return versionID + activationVersionSeparator + activationID
}

func scopeIndexKey(scope activationScopeRecord) string {
	normalized := configdomain.ActivationScope{Kind: scope.Kind, Key: scope.Key}.Normalize()
	return normalized.String()
}

func encodeConfigSetRecord(record configSetRecord) ([]byte, error) {
	return json.Marshal(record)
}

func decodeConfigSetRecord(payload []byte) (configSetRecord, error) {
	var record configSetRecord
	err := decodeJSON(payload, &record)
	return record, err
}

func encodeActivationRecord(record activationRecord) ([]byte, error) {
	return json.Marshal(record)
}

func decodeActivationRecord(payload []byte) (activationRecord, error) {
	var record activationRecord
	err := decodeJSON(payload, &record)
	return record, err
}

func encodeIngestionRuntimeRecord(record ingestionRuntimeRecord) ([]byte, error) {
	return json.Marshal(record)
}

func decodeIngestionRuntimeRecord(payload []byte) (ingestionRuntimeRecord, error) {
	var record ingestionRuntimeRecord
	err := decodeJSON(payload, &record)
	return record, err
}

func decodeJSON(payload []byte, target any) error {
	decoder := json.NewDecoder(bytes.NewReader(payload))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return err
	}
	if err := decoder.Decode(&struct{}{}); !errors.Is(err, io.EOF) {
		if err == nil {
			return fmt.Errorf("unexpected trailing data")
		}
		return err
	}
	return nil
}

func mapStorageErr(err error, message string) *problem.Problem {
	if err == nil {
		return nil
	}
	if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
		return problem.Wrap(err, problem.Unavailable, message).MarkRetryable()
	}
	return problem.Wrap(err, problem.Internal, message)
}
