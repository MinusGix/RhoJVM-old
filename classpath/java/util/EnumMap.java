package java.util;

import java.util.Map.Entry;

public class EnumMap<K extends Enum<K>, V> extends AbstractMap<K, V> implements java.io.Serializable, Cloneable {
    private final Class<K> keyClass;
    
    // TODO: This could be implemented in a significantly more efficient method
    // but for now it is merely implemented as a hash map
    private HashMap<K, V> map;
    
    public EnumMap(Class<K> keyClass) {
        this.keyClass = keyClass;
        this.map = new HashMap<K, V>();
    }
    
    public EnumMap(EnumMap<K, ? extends V> other) {
        this.keyClass = other.keyClass;
        // TODO: Is this correct?
        this.map = (HashMap<K, V>) other.map.clone();
    }
    
    public EnumMap(Map<K, ? extends V> other) {
        if (other instanceof EnumMap) {
            EnumMap<K, ? extends V> otherE = (EnumMap<K, ? extends V>) other;
            this.keyClass = otherE.keyClass;
            this.map = (HashMap<K, V>) otherE.map.clone();
        } else {
            if (other.isEmpty()) {
                throw new IllegalArgumentException("Empty map");
            }
            
            Class<K> targetClass = other.keySet().iterator().next().getDeclaringClass();
            this.keyClass = targetClass;
            this.map = new HashMap<K, V>();
            this.putAll(other);
        }
    }
    
    public void clear() {
        this.map.clear();
    }
    
    public EnumMap<K, V> clone() {
        return new EnumMap(this);
    }
    
    public boolean equals(Object other) {
        if (other instanceof EnumMap) {
            EnumMap<?, ?> otherE = (EnumMap<?, ?>) other;
            return this.keyClass.equals(otherE.keyClass) && this.map.equals(otherE.map);
        }
        return false;
    }
    
    public boolean containsKey(Object key) {
        return this.map.containsKey(key);
    }
    
    public Set<Map.Entry<K, V>> entrySet() {
        return this.map.entrySet();
    }
    
    public int size() {
        return this.map.size();
    }
    
    public boolean containsValue(Object value) {
        return this.map.containsValue(value);
    }
    
    public V get(Object key) {
        return this.map.get(key);
    }
    
    public V put(K key, V value) {
        return this.map.put(key, value);
    }
    
    /// Copy from the other map to this map
    public void putAll(Map<? extends K, ? extends V> other) {
        // TODO: Check if it is an enum map and throw an exception on non matching key classes?
        this.map.putAll(other);
    }
    
    public V remove(Object key) {
        return this.map.remove(key);
    }
    
    public Collection<V> values() {
        return this.map.values();
    }
}