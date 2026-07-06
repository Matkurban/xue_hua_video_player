// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'player_events.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$MediaSourceDto {

 String get field0;
/// Create a copy of MediaSourceDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$MediaSourceDtoCopyWith<MediaSourceDto> get copyWith => _$MediaSourceDtoCopyWithImpl<MediaSourceDto>(this as MediaSourceDto, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is MediaSourceDto&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode => Object.hash(runtimeType,field0);

@override
String toString() {
  return 'MediaSourceDto(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $MediaSourceDtoCopyWith<$Res>  {
  factory $MediaSourceDtoCopyWith(MediaSourceDto value, $Res Function(MediaSourceDto) _then) = _$MediaSourceDtoCopyWithImpl;
@useResult
$Res call({
 String field0
});




}
/// @nodoc
class _$MediaSourceDtoCopyWithImpl<$Res>
    implements $MediaSourceDtoCopyWith<$Res> {
  _$MediaSourceDtoCopyWithImpl(this._self, this._then);

  final MediaSourceDto _self;
  final $Res Function(MediaSourceDto) _then;

/// Create a copy of MediaSourceDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') @override $Res call({Object? field0 = null,}) {
  return _then(_self.copyWith(
field0: null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as String,
  ));
}

}


/// Adds pattern-matching-related methods to [MediaSourceDto].
extension MediaSourceDtoPatterns on MediaSourceDto {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( MediaSourceDto_Uri value)?  uri,TResult Function( MediaSourceDto_FlutterAsset value)?  flutterAsset,required TResult orElse(),}){
final _that = this;
switch (_that) {
case MediaSourceDto_Uri() when uri != null:
return uri(_that);case MediaSourceDto_FlutterAsset() when flutterAsset != null:
return flutterAsset(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( MediaSourceDto_Uri value)  uri,required TResult Function( MediaSourceDto_FlutterAsset value)  flutterAsset,}){
final _that = this;
switch (_that) {
case MediaSourceDto_Uri():
return uri(_that);case MediaSourceDto_FlutterAsset():
return flutterAsset(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( MediaSourceDto_Uri value)?  uri,TResult? Function( MediaSourceDto_FlutterAsset value)?  flutterAsset,}){
final _that = this;
switch (_that) {
case MediaSourceDto_Uri() when uri != null:
return uri(_that);case MediaSourceDto_FlutterAsset() when flutterAsset != null:
return flutterAsset(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String field0)?  uri,TResult Function( String field0)?  flutterAsset,required TResult orElse(),}) {final _that = this;
switch (_that) {
case MediaSourceDto_Uri() when uri != null:
return uri(_that.field0);case MediaSourceDto_FlutterAsset() when flutterAsset != null:
return flutterAsset(_that.field0);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String field0)  uri,required TResult Function( String field0)  flutterAsset,}) {final _that = this;
switch (_that) {
case MediaSourceDto_Uri():
return uri(_that.field0);case MediaSourceDto_FlutterAsset():
return flutterAsset(_that.field0);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String field0)?  uri,TResult? Function( String field0)?  flutterAsset,}) {final _that = this;
switch (_that) {
case MediaSourceDto_Uri() when uri != null:
return uri(_that.field0);case MediaSourceDto_FlutterAsset() when flutterAsset != null:
return flutterAsset(_that.field0);case _:
  return null;

}
}

}

/// @nodoc


class MediaSourceDto_Uri extends MediaSourceDto {
  const MediaSourceDto_Uri(this.field0): super._();
  

@override final  String field0;

/// Create a copy of MediaSourceDto
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$MediaSourceDto_UriCopyWith<MediaSourceDto_Uri> get copyWith => _$MediaSourceDto_UriCopyWithImpl<MediaSourceDto_Uri>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is MediaSourceDto_Uri&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode => Object.hash(runtimeType,field0);

@override
String toString() {
  return 'MediaSourceDto.uri(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $MediaSourceDto_UriCopyWith<$Res> implements $MediaSourceDtoCopyWith<$Res> {
  factory $MediaSourceDto_UriCopyWith(MediaSourceDto_Uri value, $Res Function(MediaSourceDto_Uri) _then) = _$MediaSourceDto_UriCopyWithImpl;
@override @useResult
$Res call({
 String field0
});




}
/// @nodoc
class _$MediaSourceDto_UriCopyWithImpl<$Res>
    implements $MediaSourceDto_UriCopyWith<$Res> {
  _$MediaSourceDto_UriCopyWithImpl(this._self, this._then);

  final MediaSourceDto_Uri _self;
  final $Res Function(MediaSourceDto_Uri) _then;

/// Create a copy of MediaSourceDto
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(MediaSourceDto_Uri(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class MediaSourceDto_FlutterAsset extends MediaSourceDto {
  const MediaSourceDto_FlutterAsset(this.field0): super._();
  

@override final  String field0;

/// Create a copy of MediaSourceDto
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$MediaSourceDto_FlutterAssetCopyWith<MediaSourceDto_FlutterAsset> get copyWith => _$MediaSourceDto_FlutterAssetCopyWithImpl<MediaSourceDto_FlutterAsset>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is MediaSourceDto_FlutterAsset&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode => Object.hash(runtimeType,field0);

@override
String toString() {
  return 'MediaSourceDto.flutterAsset(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $MediaSourceDto_FlutterAssetCopyWith<$Res> implements $MediaSourceDtoCopyWith<$Res> {
  factory $MediaSourceDto_FlutterAssetCopyWith(MediaSourceDto_FlutterAsset value, $Res Function(MediaSourceDto_FlutterAsset) _then) = _$MediaSourceDto_FlutterAssetCopyWithImpl;
@override @useResult
$Res call({
 String field0
});




}
/// @nodoc
class _$MediaSourceDto_FlutterAssetCopyWithImpl<$Res>
    implements $MediaSourceDto_FlutterAssetCopyWith<$Res> {
  _$MediaSourceDto_FlutterAssetCopyWithImpl(this._self, this._then);

  final MediaSourceDto_FlutterAsset _self;
  final $Res Function(MediaSourceDto_FlutterAsset) _then;

/// Create a copy of MediaSourceDto
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(MediaSourceDto_FlutterAsset(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

// dart format on
