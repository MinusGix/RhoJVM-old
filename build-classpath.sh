# Build all of java files in classpath

# Cd into classpath so that it gives them the proper path, like java/lang/Class
cd classpath
# CLASSPATH="../rhojvm/ex/lib/rt/" 
J_FILES=""
# java/lang
JFILES="$JFILES ./java/lang/Object.java ./java/lang/ClassLoader.java ./java/lang/Class.java ./java/lang/Package.java ./java/lang/System.java ./java/lang/Runtime.java ./java/lang/Thread.java ./java/lang/ThreadLocal.java ./java/lang/String.java ./java/lang/StringBuilder.java ./java/lang/Throwable.java ./java/lang/Integer.java ./java/lang/Long.java ./java/lang/Float.java ./java/lang/Double.java"
JFILE="$JFILES ./java/lang/UnsupportedOperationException.java"

# java/lang/reflect
JFILES="$JFILES ./java/lang/reflect/Field.java ./java/lang/reflect/Array.java ./java/lang/reflect/Constructor.java ./java/lang/reflect/Executable.java"

# java/lang/invoke
JFILES="$JFILES ./java/lang/invoke/MethodHandles.java ./java/lang/invoke/MethodHandle.java ./java/lang/invoke/MethodHandleInfo.java ./java/lang/invoke/MethodType.java ./java/lang/invoke/CallSite.java ./java/lang/invoke/ConstantCallSite.java ./java/lang/invoke/MutableCallSite.java ./java/lang/invoke/VolatileCallSite.java"

# java/lang/ref
JFILES="$JFILES ./java/lang/ref/Reference.java"

# java/security
JFILES="$JFILES ./java/security/AccessController.java"

# java/util
JFILES="$JFILES ./java/util/EnumMap.java"

# sun/misc/
JFILES="$JFILES ./sun/misc/VM.java ./sun/misc/Unsafe.java ./sun/misc/Perf.java ./sun/misc/PerfCounter.java"

# sun/reflect/
JFILES="$JFILES ./sun/reflect/Reflection.java"

# rho/
JFILES="$JFILES ./rho/SystemClassLoader.java ./rho/StringConversion.java ./rho/InternalField.java ./rho/SupplierThreadLocal.java"

# rho/invoke
JFILES="$JFILES ./rho/invoke/MethodHandleInst.java ./rho/invoke/MethodHandleInfoInst.java ./rho/invoke/MainMethodHandle.java"

# rho/util/
JFILES="$JFILES ./rho/util/EmptyEnumeration.java ./rho/util/SingleEnumeration.java ./rho/util/Log.java"

echo $JFILES
# Compile them all
javac -cp . $JFILES

cd ..